//! The events behind `TriggerPattern::Did`: each action of `mtg_cards::did::Action` is one arm of [`did_happened`], read off
//! the event itself or off what the batch already found about it.

use mtg_cards::ability::Ability;
use mtg_cards::assemble::mana_shape;
use mtg_cards::did::Action;
use mtg_cards::refs::{Ref, Selector};
use mtg_cards::trigger::TriggerPattern;
use mtg_core::ids::{ObjectId, PlayerId};

use super::{Happened, counted};
use crate::abilities::{AbilityRef, AbilitySource, Cause};
use crate::ability_reader;
use crate::characteristics::controller_of;
use crate::choose::Target;
use crate::events::{Event, ZoneCause};
use crate::object::GameObject;
use crate::resolve::Scope;
use crate::state::GameState;
use crate::zones::Zone;

const fn did(action: Action, player: Option<PlayerId>, object: Option<ObjectId>) -> Happened {
    Happened::Did {
        action,
        player,
        object,
        ability: None,
        amount: 0,
    }
}

/// A clash of `player`, its amount 1 when they won and 0 when they lost.
const fn clashed(player: PlayerId, won: bool) -> Happened {
    Happened::Did {
        action: Action::Clashed,
        player: Some(player),
        object: None,
        ability: None,
        amount: if won { 1 } else { 0 },
    }
}

/// The ability the activation of the `index`th ability of `source` from `origin` put on the stack: the newest such ability
/// object on the stack after the batch.
fn activated_on_stack(
    post: &GameState,
    source: ObjectId,
    index: usize,
    origin: AbilitySource,
) -> Option<ObjectId> {
    post.stack().iter().rev().copied().find(|id| {
        post.object(*id)
            .and_then(GameObject::ability)
            .is_some_and(|ability| {
                ability.source().id == source
                    && ability.index() == index
                    && ability.origin() == origin
            })
    })
}

/// Whether the activated ability has no {T} in its activation cost; a loyalty ability never has one.
fn without_tap(rules: &Ability) -> bool {
    match rules {
        Ability::Activated { cost, .. } => !cost.0.contains(&mtg_cards::cost::CostItem::Tap),
        Ability::Loyalty { .. } => true,
        _ => false,
    }
}

/// The action a move between zones is, from its cause.
const fn moved_action(cause: ZoneCause) -> Option<Action> {
    Some(match cause {
        ZoneCause::Destroyed | ZoneCause::Buried => Action::Destroyed,
        ZoneCause::Exiled
        | ZoneCause::ExiledUntil { .. }
        | ZoneCause::ExiledUntilMonarch { .. } => Action::Exiled,
        ZoneCause::Countered => Action::Countered,
        ZoneCause::Sacrificed => Action::Sacrificed,
        ZoneCause::Milled => Action::Milled,
        ZoneCause::Foretold => Action::Foretold,
        ZoneCause::Plotted => Action::Plotted,
        _ => return None,
    })
}

/// The actions `event` is, given what the batch already found about it (`fresh`). `pre` and `post` are the states before
/// and after the batch.
#[allow(clippy::too_many_lines)] // one arm for each event
pub(super) fn did_happened(
    pre: &GameState,
    post: &GameState,
    event: &Event,
    fresh: &[Happened],
) -> Vec<Happened> {
    // The player an object belongs to: its controller, or its owner off the battlefield; what left the battlefield is
    // read from the state before.
    let player_of = |id: ObjectId, back: bool| {
        let (first, second) = if back { (pre, post) } else { (post, pre) };
        controller_of(first, id)
            .or_else(|| controller_of(second, id))
            .or_else(|| {
                first
                    .object(id)
                    .or_else(|| second.object(id))
                    .map(GameObject::owner)
            })
    };
    let of =
        |action: Action, id: ObjectId| did(action, player_of(id, action.looks_back()), Some(id));
    let mut found = Vec::new();
    match event {
        Event::AbilityActivated {
            player,
            source,
            ability,
            origin,
        } => found.extend(activated(pre, post, *player, source.id, *ability, *origin)),
        Event::Enlisted { object, player, .. } => {
            found.push(did(Action::Enlisted, Some(*player), Some(*object)));
        }
        Event::ManaAbilityActivated { player, source } => {
            found.push(did(Action::ActivatedAny, Some(*player), Some(source.id)));
        }
        Event::AbilityTriggered(triggered) => found.push(did(
            Action::AbilityTriggered,
            Some(triggered.controller),
            Some(triggered.source.id),
        )),
        Event::StickerPut {
            player,
            object,
            sheet,
            sticker,
        } => {
            let at = crate::un34_stickers::StickerAt {
                player: *player,
                sheet: *sheet,
                sticker: *sticker,
            };
            if let Some(kind) = post.sticker(at).map(mtg_cards::ability::Sticker::kind) {
                let amount = crate::un34_stickers::kind_code(kind);
                for action in std::iter::once(Action::PutSticker).chain(Action::sticker_put(kind)) {
                    found.push(Happened::Did {
                        action,
                        player: Some(*player),
                        object: Some(*object),
                        ability: None,
                        amount,
                    });
                }
            }
        }
        Event::LandPlayed { player, object, .. } => {
            found.push(did(Action::PlayedLand, Some(*player), Some(*object)));
        }
        Event::Transformed { object, .. } => found.push(of(Action::Transformed, *object)),
        Event::Flipped { object } => found.push(of(Action::Flipped, *object)),
        Event::Unattached { object } => found.push(of(Action::Unattached, *object)),
        Event::Paired { object, .. } => found.push(of(Action::Paired, *object)),
        Event::Regenerated { object } => found.push(of(Action::Regenerated, *object)),
        Event::Scried(scried) => found.push(Happened::Did {
            action: Action::Scried,
            player: Some(scried.player),
            object: None,
            ability: None,
            amount: u32::try_from(scried.top.len() + scried.bottom.len()).unwrap_or(u32::MAX),
        }),
        Event::Surveilled(surveilled) => {
            found.push(did(Action::Surveilled, Some(surveilled.player), None));
        }
        Event::CardsFound(found_cards) if matches!(found_cards.zone, Zone::Library(_)) => {
            found.push(did(Action::SearchedLibrary, Some(found_cards.player), None));
            if found_cards.zone == Zone::Library(found_cards.player) {
                found.push(did(
                    Action::SearchedOwnLibrary,
                    Some(found_cards.player),
                    None,
                ));
            }
        }
        Event::LibraryShuffled { player, .. } => {
            found.push(did(Action::Shuffled, Some(*player), None));
        }
        Event::PlayerLost { player, .. } => found.push(did(Action::LostGame, Some(*player), None)),
        Event::Learned { player } => found.push(did(Action::Learned, Some(*player), None)),
        Event::EvidenceCollected { player } => {
            found.push(did(Action::CollectedEvidence, Some(*player), None));
        }
        Event::Clashed { player, won, .. } => found.push(clashed(*player, *won)),
        Event::Discovered {
            player,
            value,
            card,
        } => found.push(Happened::Did {
            action: Action::Discovered,
            player: Some(*player),
            object: *card,
            ability: None,
            amount: *value,
        }),
        Event::DreadManifested { player, card } => {
            found.push(did(Action::ManifestedDread, Some(*player), *card));
        }
        Event::GiftGiven { player, .. } => found.push(did(Action::GaveGift, Some(*player), None)),
        Event::Foraged { player } => found.push(did(Action::Foraged, Some(*player), None)),
        Event::CountersChanged(changed) => {
            if let Some(object) = crate::rmt8_turns::evolved(pre, post, changed) {
                found.push(of(Action::Evolved, object));
            }
        }
        _ => {}
    }
    found.extend(from_found(fresh, &of));
    found
}

/// The actions the batch already found about an event: moves by their cause, keyword actions, tokens and doors.
fn from_found(fresh: &[Happened], of: &dyn Fn(Action, ObjectId) -> Happened) -> Vec<Happened> {
    let mut found = Vec::new();
    for happened in fresh {
        match happened {
            Happened::Moved {
                object,
                cause: Some(cause),
                ..
            } => {
                if let Some(action) = moved_action(*cause) {
                    found.push(of(action, *object));
                }
            }
            Happened::DoorUnlocked {
                player,
                object,
                fully,
                ..
            } => {
                found.push(did(Action::Unlocked, Some(*player), Some(*object)));
                if *fully {
                    found.push(did(Action::FullyUnlocked, Some(*player), Some(*object)));
                }
            }
            Happened::Created { object, .. } => found.push(of(Action::TokenCreated, *object)),
            // The amount of an investigation is which time of the turn it is, which "for the first time" reads.
            Happened::Investigated { player, nth } => found.push(Happened::Did {
                action: Action::Investigated,
                player: Some(*player),
                object: None,
                ability: None,
                amount: *nth,
            }),
            Happened::Proliferated { player } => {
                found.push(did(Action::Proliferated, Some(*player), None));
            }
            Happened::Amassed { player } => found.push(did(Action::Amassed, Some(*player), None)),
            Happened::Bolstered { player } => {
                found.push(did(Action::Bolstered, Some(*player), None));
            }
            Happened::DiceRolled { player, .. } => {
                found.push(did(Action::Rolled, Some(*player), None));
            }
            Happened::Copied { player, spell } => {
                found.push(did(Action::CopiedSpell, Some(*player), Some(*spell)));
            }
            Happened::Explored { object, player } => {
                found.push(did(Action::Explored, Some(*player), Some(*object)));
            }
            Happened::Connived { object, player } => {
                found.push(did(Action::Connived, Some(*player), Some(*object)));
            }
            Happened::Exerted { object, player } => {
                found.push(did(Action::Exerted, Some(*player), Some(*object)));
            }
            Happened::Monstrous { object, player, .. } => {
                found.push(did(Action::BecameMonstrous, Some(*player), Some(*object)));
            }
            Happened::Renowned { object, player } => {
                found.push(did(Action::BecameRenowned, Some(*player), Some(*object)));
            }
            Happened::Exploited { object, player } => {
                found.push(did(Action::Exploited, Some(*player), Some(*object)));
            }
            Happened::Mutated { object, player } => {
                found.push(did(Action::Mutated, Some(*player), Some(*object)));
            }
            Happened::Adapted { object, player } => {
                found.push(did(Action::Adapted, Some(*player), Some(*object)));
            }
            Happened::TurnedFaceUp { object, player } => {
                found.push(did(Action::TurnedFaceUp, Some(*player), Some(*object)));
            }
            _ => {}
        }
    }
    found
}

/// What a `Did` pattern finds in `happened`: the player and the object, when the action has them, as the subjects.
pub(super) fn matching_did(
    pattern: &TriggerPattern,
    happened: &Happened,
    scope: &Scope<'_>,
) -> Option<Cause> {
    let (
        TriggerPattern::Did { action, who, what },
        Happened::Did {
            action: done,
            player,
            object,
            ability,
            amount,
        },
    ) = (pattern, happened)
    else {
        return None;
    };
    if action != done {
        return None;
    }
    let state = scope.state;
    let anyone = matches!(who, Ref::EachPlayer);
    if !anyone
        && !player.is_some_and(|player| scope.players(who).is_ok_and(|p| p.contains(&player)))
    {
        return None;
    }
    let reference = object.and_then(|id| state.object(id).map(GameObject::reference));
    if !chosen(scope, what, reference)? {
        return None;
    }
    let mut subjects: Vec<Target> = player
        .iter()
        .map(|player| Target::Player(*player))
        .collect();
    subjects.extend(reference.map(Target::Object));
    let mut cause = counted(state, subjects, *amount);
    // "Copy that ability": the ability activated did it (`Ref::Dealer`), as the spell or ability that targets does.
    cause.source = ability.and_then(|id| state.object(id).map(GameObject::reference));
    Some(cause)
}

/// The permanents on the battlefield before and after the batch whose controller differs: the controller is the result
/// of the layers (CR 613.1b), which no event carries, so it is read off the two states when an event could change it.
pub(super) fn controller_changes(
    pre: &GameState,
    post: &GameState,
    events: &[Event],
) -> Vec<Happened> {
    let may_change = events.iter().any(|event| {
        matches!(
            event,
            Event::EffectCreated { .. }
                | Event::EffectExpired { .. }
                | Event::ZoneChanged { .. }
                | Event::Attached { .. }
                | Event::Unattached { .. }
                | Event::PlayerLost { .. }
                | Event::ObjectLeftGame { .. }
        )
    });
    if !may_change {
        return Vec::new();
    }
    pre.objects()
        .filter(|object| object.zone() == Zone::Battlefield)
        .filter_map(|object| {
            let now = post.object(object.id())?;
            if now.zone() != Zone::Battlefield {
                return None;
            }
            let after = controller_of(post, object.id())?;
            (controller_of(pre, object.id())? != after)
                .then(|| did(Action::ChangedController, Some(after), Some(object.id())))
        })
        .collect()
}

/// Whether the object, if there is one, is among what the selector picks; an empty selector picks anything.
fn chosen(
    scope: &Scope<'_>,
    what: &Selector,
    reference: Option<crate::object::ObjectRef>,
) -> Option<bool> {
    if what.is_empty() {
        return Some(true);
    }
    scope.selects(what, reference?).ok()
}

/// The actions an `AbilityActivated` of the `index`th ability of `source` from `origin` is: an activation that isn't of
/// a mana ability, of any ability, of its kinds (exhaust, boast) and of a loyalty ability.
fn activated(
    pre: &GameState,
    post: &GameState,
    player: PlayerId,
    source: ObjectId,
    index: usize,
    origin: AbilitySource,
) -> Vec<Happened> {
    let address = AbilityRef {
        object: source,
        ability: index,
        source: origin,
    };
    let rules = ability_reader::ability(pre, address);
    let mana = rules.as_ref().is_some_and(|rules| rules.is_mana_ability());
    // "Without {T} in its activation cost" counts a mana ability too: a mana ability that does not tap logs this event
    // beside `ManaAbilityActivated`.
    let untapped = rules
        .as_deref()
        .is_some_and(without_tap)
        .then(|| Happened::Did {
            action: Action::ActivatedWithoutTap,
            player: Some(player),
            object: Some(source),
            ability: activated_on_stack(post, source, index, origin),
            amount: 0,
        });
    // A mana ability of a shape the payment runs logs its activation as `ManaAbilityActivated`, and this event stands
    // for it only for a limit; one of another shape is announced, and this event is its activation.
    if mana {
        let announced = rules
            .as_ref()
            .is_some_and(|rules| mana_shape(rules).is_none());
        let mut found = if announced {
            vec![did(Action::ActivatedAny, Some(player), Some(source))]
        } else {
            Vec::new()
        };
        found.extend(untapped);
        return found;
    }
    let on_stack = activated_on_stack(post, source, index, origin);
    let kinds = rules
        .as_deref()
        .map(Action::activation_kinds)
        .unwrap_or_default();
    let loyalty = rules.is_some_and(|rules| matches!(*rules, Ability::Loyalty { .. }));
    [Action::Activated, Action::ActivatedAny]
        .into_iter()
        .chain(kinds)
        .chain(loyalty.then_some(Action::ActivatedLoyalty))
        .map(|action| Happened::Did {
            action,
            player: Some(player),
            object: Some(source),
            ability: on_stack,
            amount: 0,
        })
        .chain(untapped)
        .collect()
}
