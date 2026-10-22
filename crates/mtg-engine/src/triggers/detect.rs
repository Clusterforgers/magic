//! Finding the triggered abilities that trigger on the events of a batch (CR 603.2).
//!
//! What happened is read off the events once, then every ability that waits for such a thing is tried against it.
//! A look-back event (an object leaves the battlefield, CR 603.10a) is judged against the state before the batch:
//! the abilities that worked, and the objects as they were. Every other event is judged against the state after.

use std::collections::BTreeSet;
use std::sync::Arc;

use mtg_cards::ability::{Ability, Limit};
use mtg_cards::refs::{Ref, Selector, ZoneKind};
use mtg_cards::trigger::TriggerPattern;
use mtg_cards::value::CounterKind;
use mtg_core::ids::{ObjectId, PlayerId};

use super::{DelayedId, Triggered, x_read};
use crate::abilities::{AbilityRef, AbilitySource, Cause};
use crate::ability_reader::{abilities_of, provided_here, working_here};
use crate::characteristics::{controller_of, controller_of_object};
use crate::choose::{Choices, Target};
use crate::combat::{AttackTarget, DamageForm, DamageTarget};
use crate::events::{Event, ResolutionOutcome, ZoneCause};
use crate::object::{GameObject, ObjectKind, ObjectRef};
use crate::replace::in_combat_damage;
use crate::resolve::{Scope, zone_kind};
use crate::rules::triggers_again;
use crate::snapshot::{Snapshot, snapshot};
use crate::state::{GameState, Tally};
use crate::turn::Step;
use crate::zones::Zone;

/// One thing that happened, as the patterns of triggered abilities see it.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Happened {
    /// An object moved between zones.
    Moved {
        object: ObjectId,
        from: Zone,
        to: Zone,
        cause: Option<ZoneCause>,
    },
    /// A token was created: it enters the battlefield from no zone.
    Created { object: ObjectId },
    /// A creature was declared as an attacker, the `nth` time this turn for it.
    Attacked { attacker: ObjectId, nth: u32 },
    /// A creature was declared as the only attacker.
    AttackedAlone { attacker: ObjectId },
    /// A creature was declared as an attacker of a player.
    AttackedPlayer {
        attacker: ObjectId,
        player: PlayerId,
    },
    /// Creatures were declared as attackers of a player, once for the declaration (not for a planeswalker).
    PlayerAttacked { at: PlayerId },
    /// A player declared creatures as attackers of a player, once for each pair of players in the declaration.
    PlayerAttacks { by: PlayerId, at: PlayerId },
    /// A player cast a spell, the `nth` of their turn, after `before` spells that every player cast this turn.
    Cast {
        player: PlayerId,
        spell: ObjectId,
        nth: u32,
        before: u32,
    },
    /// Something was chosen as a  of a spell or ability: the `nth` time this turn for it by what the same player
    /// controls, the `nth_any` time by what anyone controls.
    Targeted {
        target: Target,
        by: ObjectId,
        nth: u32,
        nth_any: u32,
    },
    /// A step began.
    Step { step: Step, active: PlayerId },
    /// A source dealt damage; `combat` is whether it was combat damage.
    Damaged {
        source: ObjectId,
        to: DamageTarget,
        combat: bool,
        amount: u32,
        nth: u32,
    },
    /// A player gained life, the `nth` time of their turn.
    LifeGained {
        player: PlayerId,
        amount: u32,
        nth: u32,
    },
    /// A player lost life: by an effect, by paying it or from damage (CR 120.3a).
    LifeLost { player: PlayerId, amount: u32 },
    /// A player drew `card`, the `nth` of their turn.
    Drew {
        player: PlayerId,
        card: ObjectId,
        nth: u32,
    },
    /// A permanent became tapped.
    Tapped { object: ObjectId },
    /// A permanent became untapped.
    Untapped { object: ObjectId },
    /// A player tapped a permanent for mana: once for the tap, whatever mana it made.
    TappedForMana { object: ObjectId, player: PlayerId },
    /// Counters of a kind were put on a permanent (`delta` above 0) or removed from it (below 0).
    Counters {
        on: ObjectId,
        counter: CounterKind,
        delta: i32,
    },
    /// A creature was declared as a blocker of an attacker.
    Blocked {
        blocker: ObjectId,
        attacker: ObjectId,
    },
    /// An attacker became blocked, once however many creatures block it.
    BecameBlocked { attacker: ObjectId },
    /// Attackers were declared together.
    AttackedTogether { attackers: Vec<ObjectId> },
    /// An Aura, Equipment or Fortification became attached to a permanent.
    Attached { attachment: ObjectId, to: ObjectId },
    /// A player cycled a card.
    Cycled { player: PlayerId, card: ObjectId },
    /// A player became the monarch.
    BecameMonarch { player: PlayerId },
    /// A player proliferated.
    Proliferated { player: PlayerId },
    /// A creature explored.
    Explored { object: ObjectId, player: PlayerId },
    /// A creature connived.
    Connived { object: ObjectId, player: PlayerId },
    /// A player amassed.
    Amassed { player: PlayerId },
    /// A creature became monstrous.
    Monstrous { object: ObjectId, player: PlayerId },
    /// A permanent was turned face up.
    TurnedFaceUp { object: ObjectId, player: PlayerId },
    /// A creature adapted.
    Adapted { object: ObjectId, player: PlayerId },
    /// A player bolstered.
    Bolstered { player: PlayerId },
    /// Everything of one kind of state (before or after) that happened in the batch, for the patterns of events that
    /// happen together.
    Batch(Vec<Happened>),
}

impl Happened {
    /// Whether the abilities and objects it concerns are those of the state before it (CR 603.10a).
    const fn looks_back(&self) -> bool {
        matches!(
            self,
            Happened::Moved {
                from: Zone::Battlefield,
                ..
            }
        )
    }

    /// How much it was: the damage dealt, the life gained or lost, the counters put or removed; zero for the rest.
    const fn amount(&self) -> u32 {
        match self {
            Happened::Damaged { amount, .. }
            | Happened::LifeGained { amount, .. }
            | Happened::LifeLost { amount, .. } => *amount,
            Happened::Counters { delta, .. } => delta.unsigned_abs(),
            Happened::Cast { before, .. } => *before,
            _ => 0,
        }
    }

    /// Which time of its turn this was, for the events a turn counts; for a target, by what the same player controls.
    const fn nth(&self) -> Option<u32> {
        match self {
            Happened::Drew { nth, .. }
            | Happened::Cast { nth, .. }
            | Happened::Attacked { nth, .. }
            | Happened::Targeted { nth, .. }
            | Happened::Damaged { nth, .. }
            | Happened::LifeGained { nth, .. } => Some(*nth),
            _ => None,
        }
    }

    /// The same for the event a pattern counts: a target counted by what anyone controls when the pattern names no one.
    fn nth_for(&self, event: &TriggerPattern) -> Option<u32> {
        match (self, event) {
            (Happened::Targeted { nth_any, .. }, TriggerPattern::BecomesTarget { by, .. })
                if by.is_empty() =>
            {
                Some(*nth_any)
            }
            _ => self.nth(),
        }
    }
}

/// The move between zones that `event` is, if it is one.
///
/// `pre` is the state before the batch and `post` the state after. A permanent spell that resolves enters the
/// battlefield and any other spell goes to a graveyard; a spell that does not resolve goes to its owner's graveyard
/// (an ability just ceases to exist).
fn moved(pre: &GameState, event: &Event, post: &GameState) -> Option<Happened> {
    let owner = |object: &ObjectId| pre.object(*object).map(GameObject::owner);
    let (object, from, to, cause) = match event {
        Event::LandPlayed { player, object, .. } => {
            let from = pre
                .object(*object)
                .map_or(Zone::Hand(*player), GameObject::zone);
            (*object, from, Zone::Battlefield, None)
        }
        Event::SpellResolved { object } => {
            let landed = post.object(*object)?;
            (*object, Zone::Stack, landed.zone(), None)
        }
        Event::CreatureDied { object, .. } => (
            *object,
            Zone::Battlefield,
            Zone::Graveyard(owner(object)?),
            None,
        ),
        Event::CardDiscarded { player, object } => (
            *object,
            Zone::Hand(*player),
            Zone::Graveyard(owner(object)?),
            Some(ZoneCause::Discarded),
        ),
        Event::CardDrawn { player, object } => {
            (*object, Zone::Library(*player), Zone::Hand(*player), None)
        }
        Event::ZoneChanged {
            object,
            from,
            to,
            cause,
        } => (*object, *from, *to, Some(*cause)),
        Event::ResolutionEnded { object, outcome } if *outcome != ResolutionOutcome::Resolved => {
            let spell = pre
                .object(*object)
                .filter(|o| o.kind() != ObjectKind::Ability)?;
            // A spell a replacement effect sent elsewhere was moved by the event that did it.
            let landed = post.object(*object)?.zone();
            if !matches!(landed, Zone::Graveyard(_)) {
                return None;
            }
            (
                *object,
                Zone::Stack,
                Zone::Graveyard(spell.owner()),
                Some(ZoneCause::Countered),
            )
        }
        _ => return None,
    };
    Some(Happened::Moved {
        object,
        from,
        to,
        cause,
    })
}

/// What happened in `events`, in order. `pre` is the state before them and `post` the state after.
fn happenings(pre: &GameState, events: &[Event], post: &GameState) -> Vec<Happened> {
    let mut found = Vec::new();
    // Permanents tapped in this batch whose mana has not been added yet: tapping for mana is the tap that adds it.
    let mut tapped = Vec::new();
    for (at, event) in events.iter().enumerate() {
        let later = &events[at + 1..];
        let look = Look { pre, post, later };
        if let Some(moved) = moved(pre, event, post) {
            found.push(moved);
        }
        match event {
            Event::TokenCreated(token) => found.push(Happened::Created {
                object: token.object,
            }),
            Event::AttackersDeclared { attackers, .. } => {
                found.extend(attackers.iter().map(|(attacker, _)| Happened::Attacked {
                    attacker: *attacker,
                    nth: before(look, *attacker).map_or(0, |object| {
                        object_ordinal(look, object, None, Tally::Attacked)
                    }),
                }));
                if let [(attacker, _)] = attackers.as_slice() {
                    found.push(Happened::AttackedAlone {
                        attacker: *attacker,
                    });
                }
                found.extend(attackers.iter().filter_map(|(attacker, at)| match at {
                    AttackTarget::Player(player) => Some(Happened::AttackedPlayer {
                        attacker: *attacker,
                        player: *player,
                    }),
                    AttackTarget::Planeswalker(_) => None,
                }));
                let mut pairs: Vec<(PlayerId, PlayerId)> = Vec::new();
                for (attacker, at) in attackers {
                    if let AttackTarget::Player(at) = at
                        && let Some(by) =
                            controller_of(post, *attacker).or_else(|| controller_of(pre, *attacker))
                        && !pairs.contains(&(by, *at))
                    {
                        pairs.push((by, *at));
                    }
                }
                for (_, at) in &pairs {
                    if !found.contains(&Happened::PlayerAttacked { at: *at }) {
                        found.push(Happened::PlayerAttacked { at: *at });
                    }
                }
                found.extend(
                    pairs
                        .into_iter()
                        .map(|(by, at)| Happened::PlayerAttacks { by, at }),
                );
                if !attackers.is_empty() {
                    found.push(Happened::AttackedTogether {
                        attackers: attackers.iter().map(|(attacker, _)| *attacker).collect(),
                    });
                }
            }
            Event::SpellCast {
                player,
                object,
                choices,
            } => {
                let nth = ordinal(post, later, *player, Tally::Cast);
                found.push(Happened::Cast {
                    player: *player,
                    spell: *object,
                    nth,
                    before: cast_before(post, later),
                });
                found.extend(targeted(look, choices, (*object, *player)));
            }
            Event::AbilityPutOnStack(put) => {
                found.extend(targeted(look, &put.choices, (put.object, put.controller)));
            }
            Event::SpellCopied(copy) => {
                found.extend(targeted(
                    look,
                    &copy.choices,
                    (copy.object, copy.controller),
                ));
            }
            Event::AbilityCopied(copy) => {
                found.extend(targeted(
                    look,
                    &copy.choices,
                    (copy.object, copy.controller),
                ));
            }
            Event::StepBegan { step } => found.push(Happened::Step {
                step: *step,
                active: post.turn().active(),
            }),
            other => about_players_and_permanents(other, look, &mut tapped, &mut found),
        }
    }
    found
}

/// How many spells every player cast this turn before the one the event casts: the count after the batch less that spell
/// and the casts that follow it in the batch (`later`).
fn cast_before(post: &GameState, later: &[Event]) -> u32 {
    let total: u32 = post
        .players()
        .iter()
        .map(|player| post.tallied(player.id(), Tally::Cast))
        .sum();
    let after = later
        .iter()
        .filter(|event| matches!(event, Event::SpellCast { .. }))
        .count();
    total.saturating_sub(u32::try_from(after).unwrap_or(u32::MAX).saturating_add(1))
}

/// Which time of the turn the event of `player` and kind `kind` is: the count after the batch less the events of that kind
/// that follow it in the batch (`later`).
fn ordinal(post: &GameState, later: &[Event], player: PlayerId, kind: Tally) -> u32 {
    let same = |event: &Event| match (kind, event) {
        (Tally::Drawn, Event::CardDrawn { player: who, .. })
        | (Tally::Cast, Event::SpellCast { player: who, .. })
        | (Tally::Gained, Event::LifeGained { player: who, .. }) => *who == player,
        _ => false,
    };
    let after = post.tallied(player, kind);
    after
        .saturating_sub(u32::try_from(later.iter().filter(|e| same(e)).count()).unwrap_or(u32::MAX))
}

/// Where an event stands in its batch: the state before the batch, the state after it and the events that follow the event.
#[derive(Clone, Copy)]
struct Look<'a> {
    pre: &'a GameState,
    post: &'a GameState,
    later: &'a [Event],
}

/// The object `id` as it was before the batch, which is how the turn's tallies name it.
fn before(look: Look<'_>, id: ObjectId) -> Option<ObjectRef> {
    let object = look.pre.object(id).or_else(|| look.post.object(id));
    object.map(GameObject::reference)
}

/// Which time of the turn the event of `kind` is for `object` (caused by what `by` controls, when it is given): the count
/// after the batch less the events of that kind for the object that follow it in the batch.
fn object_ordinal(look: Look<'_>, object: ObjectRef, by: Option<PlayerId>, kind: Tally) -> u32 {
    let by_them = |player: PlayerId| by.is_none_or(|by| by == player);
    let same = |event: &Event| match (kind, event) {
        (Tally::Attacked, Event::AttackersDeclared { attackers, .. }) => {
            attackers.iter().any(|(attacker, _)| *attacker == object.id)
        }
        (
            Tally::Damaged,
            Event::DamageDealt {
                target: DamageTarget::Object(hit),
                amount,
                ..
            },
        ) => *hit == object.id && *amount > 0,
        (
            Tally::Targeted,
            Event::SpellCast {
                player, choices, ..
            },
        ) => by_them(*player) && choices.targeted_objects().contains(&object),
        (Tally::Targeted, Event::AbilityPutOnStack(put)) => {
            by_them(put.controller) && put.choices.targeted_objects().contains(&object)
        }
        (Tally::Targeted, Event::SpellCopied(copy)) => {
            by_them(copy.controller) && copy.choices.targeted_objects().contains(&object)
        }
        (Tally::Targeted, Event::AbilityCopied(copy)) => {
            by_them(copy.controller) && copy.choices.targeted_objects().contains(&object)
        }
        _ => false,
    };
    let later = look.later.iter().filter(|event| same(event)).count();
    let after = look.post.object_tallied(object, by, kind);
    after.saturating_sub(u32::try_from(later).unwrap_or(u32::MAX))
}

/// The targets chosen for the spell or ability `by`, which `controller` controls, each once.
fn targeted(
    look: Look<'_>,
    choices: &Choices,
    (by, controller): (ObjectId, PlayerId),
) -> Vec<Happened> {
    let mut chosen: Vec<Target> = Vec::new();
    for target in choices.targets.iter().flatten() {
        if !chosen.contains(target) {
            chosen.push(*target);
        }
    }
    let ordinals = |object: ObjectRef| {
        let of = |by| object_ordinal(look, object, by, Tally::Targeted);
        (of(Some(controller)), of(None))
    };
    chosen
        .into_iter()
        .map(|target| {
            let (nth, nth_any) = target.object().map_or((0, 0), ordinals);
            Happened::Targeted {
                target,
                by,
                nth,
                nth_any,
            }
        })
        .collect()
}

/// What `event` is, when it is about blocks, damage, life, cards drawn or cycled, taps, counters or attachments.
/// `tapped` holds the permanents of the batch tapped so far whose mana has not been added.
#[allow(clippy::too_many_lines)]
fn about_players_and_permanents(
    event: &Event,
    look: Look<'_>,
    tapped: &mut Vec<ObjectId>,
    found: &mut Vec<Happened>,
) {
    match event {
        Event::BlockersDeclared { blocks, .. } => {
            found.extend(blocks.iter().map(|(blocker, attacker)| Happened::Blocked {
                blocker: *blocker,
                attacker: *attacker,
            }));
            let mut blocked = Vec::new();
            for (_, attacker) in blocks {
                if !blocked.contains(attacker) {
                    blocked.push(*attacker);
                    found.push(Happened::BecameBlocked {
                        attacker: *attacker,
                    });
                }
            }
        }
        Event::DamageDealt {
            source,
            target,
            amount,
            form,
            ..
        } => {
            found.push(Happened::Damaged {
                source: *source,
                to: *target,
                combat: in_combat_damage(look.post),
                nth: match target {
                    DamageTarget::Object(hit) if *amount > 0 => before(look, *hit)
                        .map_or(0, |object| {
                            object_ordinal(look, object, None, Tally::Damaged)
                        }),
                    _ => 0,
                },
                amount: *amount,
            });
            // Damage to a player makes them lose that much life (CR 120.3a), unless it is infect (poison counters, CR 120.3b).
            if let DamageTarget::Player(player) = target
                && !matches!(form, DamageForm::Infect)
            {
                found.push(Happened::LifeLost {
                    player: *player,
                    amount: *amount,
                });
            }
        }
        Event::LifeGained { player, amount } => found.push(Happened::LifeGained {
            player: *player,
            amount: *amount,
            nth: ordinal(look.post, look.later, *player, Tally::Gained),
        }),
        Event::LifeLost { player, amount } | Event::LifePaid { player, amount } => {
            found.push(Happened::LifeLost {
                player: *player,
                amount: *amount,
            });
        }
        Event::CardDrawn { player, object } => found.push(Happened::Drew {
            player: *player,
            card: *object,
            nth: ordinal(look.post, look.later, *player, Tally::Drawn),
        }),
        Event::PermanentTapped { object } => {
            tapped.push(*object);
            found.push(Happened::Tapped { object: *object });
        }
        Event::PermanentUntapped { object } => found.push(Happened::Untapped { object: *object }),
        Event::ManaAdded {
            player,
            source: Some(source),
            ..
        } => {
            if let Some(at) = tapped.iter().position(|object| object == source) {
                tapped.remove(at);
                found.push(Happened::TappedForMana {
                    object: *source,
                    player: *player,
                });
            }
        }
        Event::CountersChanged(changed) => {
            if let Target::Object(on) = changed.on {
                found.push(Happened::Counters {
                    on: on.id,
                    counter: changed.counter.clone(),
                    delta: changed.delta,
                });
            }
        }
        Event::Attached { object, to } => found.push(Happened::Attached {
            attachment: *object,
            to: *to,
        }),
        Event::Cycled { player, object } => found.push(Happened::Cycled {
            player: *player,
            card: *object,
        }),
        Event::MonarchChanged { player } => found.push(Happened::BecameMonarch { player: *player }),
        Event::Proliferated { player } => found.push(Happened::Proliferated { player: *player }),
        Event::Amassed { player, .. } => found.push(Happened::Amassed { player: *player }),
        Event::Bolstered { player, .. } => found.push(Happened::Bolstered { player: *player }),
        Event::Monstrous { object, player } => found.push(Happened::Monstrous {
            object: *object,
            player: *player,
        }),
        Event::TurnedFaceUp { object, player } => found.push(Happened::TurnedFaceUp {
            object: *object,
            player: *player,
        }),
        Event::Adapted { object, player } => found.push(Happened::Adapted {
            object: *object,
            player: *player,
        }),
        Event::Explored { object, player } => found.push(Happened::Explored {
            object: *object,
            player: *player,
        }),
        Event::Connived { object, player } => found.push(Happened::Connived {
            object: *object,
            player: *player,
        }),
        _ => {}
    }
}

/// What the batch has used up so far, so that two events of one batch cannot use an ability up twice.
#[derive(Debug, Default)]
struct Used {
    limits: BTreeSet<(ObjectRef, AbilitySource, usize)>,
    delayed: BTreeSet<DelayedId>,
}

/// The cause of a trigger about `subjects`, with the objects among them as they are in `state`.
fn cause(state: &GameState, subjects: Vec<Target>) -> Cause {
    counted(state, subjects, 0)
}

/// The same for an event of the given amount (`Amount::Triggering`).
fn counted(state: &GameState, subjects: Vec<Target>, amount: u32) -> Cause {
    let snapshots = subjects
        .iter()
        .filter_map(|subject| subject.object())
        .filter_map(|object| snapshot(state, object.id))
        .collect();
    Cause {
        subjects,
        snapshots,
        amount,
    }
}

/// What `pattern` finds in `happened`, judged in the state of `scope`, as the cause of the trigger.
// One arm for each pair of a pattern and an event; they share the state and the closures above them.
#[allow(clippy::too_many_lines)]
fn matching(pattern: &TriggerPattern, happened: &Happened, scope: &Scope<'_>) -> Option<Cause> {
    let state = scope.state;
    let object = |id: ObjectId| state.object(id).map(GameObject::reference);
    match (pattern, happened) {
        (TriggerPattern::AnyOf(patterns), _) => patterns
            .iter()
            .find_map(|inner| matching(inner, happened, scope)),
        (
            TriggerPattern::ZoneChange {
                what,
                from: left,
                to: entered,
            },
            Happened::Moved {
                object: moved,
                from,
                to,
                ..
            },
        ) => {
            let named = |kind: &Option<ZoneKind>, zone: Zone| {
                kind.is_none_or(|kind| zone_kind(zone) == kind)
            };
            let reference = object(*moved)?;
            (named(left, *from) && named(entered, *to) && scope.selects(what, reference).ok()?)
                .then(|| cause(state, vec![Target::Object(reference)]))
        }
        (
            TriggerPattern::ZoneChange {
                what,
                from: None,
                to: entered,
            },
            Happened::Created { object: made },
        ) => {
            let reference = object(*made)?;
            (entered.is_none_or(|kind| kind == ZoneKind::Battlefield)
                && scope.selects(what, reference).ok()?)
            .then(|| cause(state, vec![Target::Object(reference)]))
        }
        (TriggerPattern::Attacks { what }, Happened::Attacked { attacker, .. }) => {
            let reference = object(*attacker)?;
            scope
                .selects(what, reference)
                .ok()?
                .then(|| cause(state, vec![Target::Object(reference)]))
        }
        (TriggerPattern::AttacksAlone { what }, Happened::AttackedAlone { attacker }) => {
            let reference = object(*attacker)?;
            scope
                .selects(what, reference)
                .ok()?
                .then(|| cause(state, vec![Target::Object(reference)]))
        }
        (
            TriggerPattern::AttacksPlayer { what, at },
            Happened::AttackedPlayer { attacker, player },
        ) => {
            let reference = object(*attacker)?;
            (scope.players(at).ok()?.contains(player) && scope.selects(what, reference).ok()?)
                .then(|| cause(state, vec![Target::Object(reference)]))
        }
        (TriggerPattern::PlayerAttacked { who }, Happened::PlayerAttacked { at }) => scope
            .players(who)
            .ok()?
            .contains(at)
            .then(|| cause(state, vec![Target::Player(*at)])),
        (
            TriggerPattern::PlayerAttacks { by, at },
            Happened::PlayerAttacks {
                by: attacking,
                at: defending,
            },
        ) => (scope.players(by).ok()?.contains(attacking)
            && scope.players(at).ok()?.contains(defending))
        .then(|| cause(state, vec![Target::Player(*attacking)])),
        (TriggerPattern::SpellCast { what, by }, Happened::Cast { player, spell, .. }) => {
            let reference = object(*spell)?;
            let subjects = vec![Target::Player(*player), Target::Object(reference)];
            (scope.players(by).ok()?.contains(player) && scope.selects(what, reference).ok()?)
                .then(|| counted(state, subjects, happened.amount()))
        }
        (
            TriggerPattern::Sacrificed { what, by },
            Happened::Moved {
                object: moved,
                from: Zone::Battlefield,
                cause: Some(ZoneCause::Sacrificed),
                ..
            },
        ) => {
            let reference = object(*moved)?;
            let sacrificer = controller_of(state, *moved)?;
            let subjects = vec![Target::Object(reference), Target::Player(sacrificer)];
            (scope.players(by).ok()?.contains(&sacrificer)
                && scope.selects(what, reference).ok()?)
            .then(|| cause(state, subjects))
        }
        (
            TriggerPattern::StepBegins { step, whose },
            Happened::Step {
                step: began,
                active,
            },
        ) => {
            if !step_matches(*step, *began) {
                None
            } else if scope.players(whose).ok()?.contains(active) {
                Some(cause(state, vec![Target::Player(*active)]))
            } else if whose.is_each_player() && !state.is_alive(*active) {
                // The steps of a turn without an active player still begin (CR 800.4) but name no player.
                Some(cause(state, Vec::new()))
            } else {
                None
            }
        }
        (
            TriggerPattern::BecomesTarget { what, by },
            Happened::Targeted {
                target, by: caster, ..
            },
        ) => becomes_target((what, by), (target, *caster), scope),
        _ => matching_event(pattern, happened, scope),
    }
}

/// An object (`target`) chosen as a target of the spell or ability `caster`, judged against what the pattern (`what`
/// is targeted, `by` what) names. The subjects of the cause are the object, what targets it and its controller.
fn becomes_target(
    (what, by): (&Selector, &Selector),
    (target, caster): (&Target, ObjectId),
    scope: &Scope<'_>,
) -> Option<Cause> {
    let state = scope.state;
    let Target::Object(targeted) = target else {
        return None;
    };
    let targeting = state.object(caster).map(GameObject::reference)?;
    let subjects = vec![
        Target::Object(*targeted),
        Target::Object(targeting),
        Target::Player(controller_of(state, caster)?),
    ];
    (scope.selects(what, *targeted).ok()? && scope.selects(by, targeting).ok()?)
        .then(|| cause(state, subjects))
}

/// What `pattern` finds in the events about a player or a permanent: damage, life, cards drawn, discarded and cycled,
/// taps, counters, blocks and attachments. The subjects of the cause (`Ref::Triggering`) are the recipient of damage and
/// the player who tapped for mana alone, so that damage to "that player" hits no creature that dealt it or land that was
/// tapped; the player and the card of a discard and of a cycling; the blocker and the attacker; the attachment and what
/// it is attached to.
fn matching_event(
    pattern: &TriggerPattern,
    happened: &Happened,
    scope: &Scope<'_>,
) -> Option<Cause> {
    let state = scope.state;
    let object = |id: ObjectId| state.object(id).map(GameObject::reference);
    let by = |who: &Ref, player: &PlayerId| scope.players(who).is_ok_and(|p| p.contains(player));
    let picks = |what: &Selector, reference: ObjectRef| scope.selects(what, reference).ok();
    match (pattern, happened) {
        (TriggerPattern::Nth { event, nth }, happened) => (happened.nth_for(event) == Some(*nth))
            .then(|| matching(event, happened, scope))
            .flatten(),
        (
            TriggerPattern::DealsDamage { source, to, combat },
            Happened::Damaged {
                source: dealer,
                to: hit,
                combat: was,
                amount,
                ..
            },
        ) => {
            let dealer = object(*dealer)?;
            let recipient = match hit {
                DamageTarget::Player(player) => Target::Player(*player),
                DamageTarget::Object(hit) => Target::Object(object(*hit)?),
            };
            let named = to
                .as_ref()
                .is_none_or(|to| scope.refs(to).is_ok_and(|named| named.contains(&recipient)));
            (combat.is_none_or(|wanted| wanted == *was) && named && picks(source, dealer)?)
                .then(|| counted(state, vec![recipient], *amount))
        }
        (TriggerPattern::LifeGained { who }, Happened::LifeGained { player, .. })
        | (TriggerPattern::LifeLost { who }, Happened::LifeLost { player, .. })
        | (TriggerPattern::BecomesMonarch { who }, Happened::BecameMonarch { player })
        | (TriggerPattern::Proliferated { who }, Happened::Proliferated { player })
        | (TriggerPattern::Amassed { who }, Happened::Amassed { player })
        | (TriggerPattern::Bolstered { who }, Happened::Bolstered { player }) => by(who, player)
            .then(|| counted(state, vec![Target::Player(*player)], happened.amount())),
        (TriggerPattern::Drew { who, what }, Happened::Drew { player, card, .. }) => {
            // The card is read where the batch left it: in the hand it was drawn into, or wherever an effect sent it since.
            let drawn = object(*card)?;
            (by(who, player) && picks(what, drawn)?)
                .then(|| counted(state, vec![Target::Player(*player)], happened.amount()))
        }
        (
            TriggerPattern::Discarded { what, by: who },
            Happened::Moved {
                object: card,
                from: Zone::Hand(player),
                cause: Some(ZoneCause::Discarded),
                ..
            },
        )
        | (TriggerPattern::Cycled { what, by: who }, Happened::Cycled { player, card }) => {
            let card = object(*card)?;
            (by(who, player) && picks(what, card)?)
                .then(|| cause(state, vec![Target::Player(*player), Target::Object(card)]))
        }
        _ => matching_permanents(pattern, happened, scope),
    }
}

/// What `pattern` finds in the events about permanents: taps, counters, blocks, attackers and attachments.
#[allow(clippy::too_many_lines)]
fn matching_permanents(
    pattern: &TriggerPattern,
    happened: &Happened,
    scope: &Scope<'_>,
) -> Option<Cause> {
    let state = scope.state;
    let object = |id: ObjectId| state.object(id).map(GameObject::reference);
    let by = |who: &Ref, player: &PlayerId| scope.players(who).is_ok_and(|p| p.contains(player));
    let picks = |what: &Selector, reference: ObjectRef| scope.selects(what, reference).ok();
    let of_kind = |wanted: &Option<CounterKind>, kind: &CounterKind| {
        wanted.as_ref().is_none_or(|wanted| wanted == kind)
    };
    match (pattern, happened) {
        (
            TriggerPattern::Explored { what },
            Happened::Explored {
                object: actor,
                player,
            },
        )
        | (
            TriggerPattern::Connived { what },
            Happened::Connived {
                object: actor,
                player,
            },
        )
        | (
            TriggerPattern::BecomesMonstrous { what },
            Happened::Monstrous {
                object: actor,
                player,
            },
        )
        | (
            TriggerPattern::TurnedFaceUp { what },
            Happened::TurnedFaceUp {
                object: actor,
                player,
            },
        )
        | (
            TriggerPattern::Adapted { what },
            Happened::Adapted {
                object: actor,
                player,
            },
        ) => {
            let actor = object(*actor)?;
            picks(what, actor)?
                .then(|| cause(state, vec![Target::Player(*player), Target::Object(actor)]))
        }
        (TriggerPattern::Tapped { what }, Happened::Tapped { object: tapped })
        | (TriggerPattern::Untapped { what }, Happened::Untapped { object: tapped }) => {
            let tapped = object(*tapped)?;
            picks(what, tapped)?.then(|| cause(state, vec![Target::Object(tapped)]))
        }
        (
            TriggerPattern::TappedForMana { what, by: who },
            Happened::TappedForMana {
                object: tapped,
                player,
            },
        ) => {
            let tapped = object(*tapped)?;
            (by(who, player) && picks(what, tapped)?)
                .then(|| cause(state, vec![Target::Player(*player)]))
        }
        (
            TriggerPattern::CountersPut { what, counter }
            | TriggerPattern::CountersRemoved { what, counter },
            Happened::Counters {
                on,
                counter: kind,
                delta,
            },
        ) => {
            let putting = matches!(pattern, TriggerPattern::CountersPut { .. });
            let on = object(*on)?;
            ((*delta > 0) == putting && of_kind(counter, kind) && picks(what, on)?)
                .then(|| counted(state, vec![Target::Object(on)], happened.amount()))
        }
        (TriggerPattern::Blocks { what }, Happened::Blocked { blocker, attacker }) => {
            let (blocker, attacker) = (object(*blocker)?, object(*attacker)?);
            picks(what, blocker)?.then(|| {
                cause(
                    state,
                    vec![Target::Object(blocker), Target::Object(attacker)],
                )
            })
        }
        (TriggerPattern::BecomesBlocked { what }, Happened::BecameBlocked { attacker }) => {
            let attacker = object(*attacker)?;
            picks(what, attacker)?.then(|| cause(state, vec![Target::Object(attacker)]))
        }
        (
            TriggerPattern::GroupAttacks { what, at_least },
            Happened::AttackedTogether { attackers },
        ) => {
            let mut picked = Vec::new();
            for attacker in attackers {
                if let Some(attacker) = object(*attacker)
                    && picks(what, attacker)?
                {
                    picked.push(Target::Object(attacker));
                }
            }
            (picked.len() >= usize::try_from(*at_least).unwrap_or(usize::MAX)).then(|| {
                let count = u32::try_from(picked.len()).unwrap_or(u32::MAX);
                counted(state, picked, count)
            })
        }
        (
            TriggerPattern::Attached { what, to },
            Happened::Attached {
                attachment,
                to: host,
            },
        ) => {
            let (attachment, host) = (object(*attachment)?, object(*host)?);
            (picks(what, attachment)? && picks(to, host)?).then(|| {
                cause(
                    state,
                    vec![Target::Object(attachment), Target::Object(host)],
                )
            })
        }
        _ => None,
    }
}

/// Whether the step a pattern names is the step that began (the two enums list the same thirteen steps).
fn step_matches(named: mtg_cards::trigger::Step, began: Step) -> bool {
    use mtg_cards::trigger::Step as Named;
    matches!(
        (named, began),
        (Named::Untap, Step::Untap)
            | (Named::Upkeep, Step::Upkeep)
            | (Named::Draw, Step::Draw)
            | (Named::Main1, Step::Main1)
            | (Named::BeginCombat, Step::BeginCombat)
            | (Named::DeclareAttackers, Step::DeclareAttackers)
            | (Named::DeclareBlockers, Step::DeclareBlockers)
            | (Named::FirstStrikeDamage, Step::FirstStrikeDamage)
            | (Named::CombatDamage, Step::CombatDamage)
            | (Named::EndCombat, Step::EndCombat)
            | (Named::Main2, Step::Main2)
            | (Named::End, Step::End)
            | (Named::Cleanup, Step::Cleanup)
    )
}

/// What `pattern` finds in `happened`: one cause for an event, and for the events of a batch one for each player they
/// are about (or one in all when they are about none).
fn causes(pattern: &TriggerPattern, happened: &Happened, scope: &Scope<'_>) -> Vec<Cause> {
    match (pattern, happened) {
        (TriggerPattern::Batch(inner), Happened::Batch(members)) => {
            let mut groups: Vec<(Vec<Target>, Cause)> = Vec::new();
            for member in members {
                let Some(found) = matching(inner, member, scope) else {
                    continue;
                };
                let players: Vec<Target> = found
                    .subjects
                    .iter()
                    .filter(|subject| matches!(subject, Target::Player(_)))
                    .copied()
                    .collect();
                let at = groups.iter().position(|(key, _)| *key == players);
                let at = at.unwrap_or_else(|| {
                    groups.push((players, Cause::default()));
                    groups.len() - 1
                });
                let group = &mut groups[at].1;
                for subject in found.subjects {
                    if !group.subjects.contains(&subject) {
                        group.subjects.push(subject);
                    }
                }
                for shot in found.snapshots {
                    if !group.snapshots.contains(&shot) {
                        group.snapshots.push(shot);
                    }
                }
                group.amount = group.amount.saturating_add(1);
            }
            groups.into_iter().map(|(_, cause)| cause).collect()
        }
        (TriggerPattern::Batch(_), _) | (_, Happened::Batch(_)) => Vec::new(),
        _ => matching(pattern, happened, scope).into_iter().collect(),
    }
}

/// The triggered abilities of `object`, which is in the zone `zone`, that trigger on `happened`: `abilities` are those
/// that work there. A permanent's abilities may trigger again (`repeats`); a card elsewhere's do not.
fn object_triggers(
    state: &GameState,
    object: &GameObject,
    zone: ZoneKind,
    abilities: impl IntoIterator<Item = (AbilityRef, Arc<Ability>)>,
    happened: &Happened,
    found: &mut Vec<Triggered>,
    used: &mut Used,
) {
    // Most objects have no triggered ability, and a snapshot costs the layers, so it is only taken for a trigger.
    let mut shot = None;
    let (source, controller) = (object.reference(), controller_of_object(state, object));
    if !state.is_alive(controller) {
        return;
    }
    for (address, rules) in abilities {
        let Ability::Triggered {
            when,
            intervening_if,
            limit,
            from,
            ..
        } = &*rules
        else {
            continue;
        };
        if *from != zone {
            continue;
        }
        let scope = Scope::announcing(state, controller, source);
        for cause in causes(when, happened, &scope) {
            // CR 603.4: an "if" in the trigger is checked as the event happens.
            if let Some(condition) = intervening_if {
                if shot.is_none() {
                    shot = snapshot(state, object.id());
                }
                let Some(held) = &shot else {
                    continue;
                };
                // The "if" reads the X the trigger will remember as it goes on the stack.
                let announced = Choices {
                    x: x_read(when, false, held, &cause),
                    ..Choices::default()
                };
                let mut scope = Scope::triggered(state, controller, source, &cause);
                scope.choices = &announced;
                if !scope.holds(condition).unwrap_or(false) {
                    continue;
                }
            }
            if *limit == Some(Limit::OncePerTurn)
                && (state.limit_spent(source, address.source, address.ability)
                    || !used
                        .limits
                        .insert((source, address.source, address.ability)))
            {
                continue;
            }
            if shot.is_none() {
                shot = snapshot(state, object.id());
            }
            let Some(shot) = &shot else {
                continue;
            };
            let trigger = Triggered {
                source,
                ability: address.ability,
                origin: address.source,
                controller,
                cause,
                snapshot: shot.clone(),
                delayed: None,
            };
            // An ability that triggers only once each turn does so once, whatever else is said of it.
            if limit.is_none() && zone == ZoneKind::Battlefield {
                for _ in 0..repeats(state, happened, controller) {
                    found.push(trigger.clone());
                }
            }
            found.push(trigger);
        }
    }
}

/// The triggered abilities of the permanents of `state` that trigger on `happened`.
fn permanents(state: &GameState, happened: &Happened, found: &mut Vec<Triggered>, used: &mut Used) {
    for &id in state.zone(Zone::Battlefield) {
        let Some(object) = state.object(id) else {
            continue;
        };
        object_triggers(
            state,
            object,
            ZoneKind::Battlefield,
            abilities_of(state, id),
            happened,
            found,
            used,
        );
    }
}

/// The triggered abilities of the cards of `state` that are not on the battlefield and work from the zone they are in
/// (a graveyard, a hand, exile, the command zone, a library, the stack) that trigger on `happened`, in id order. They
/// are judged against the same state as the permanents, so a card in the graveyard before a creature dies is there when
/// that death is looked back at (CR 603.10a).
fn elsewhere(state: &GameState, happened: &Happened, found: &mut Vec<Triggered>, used: &mut Used) {
    if !state.possible.elsewhere {
        return;
    }
    for object in state.objects() {
        if object.zone() == Zone::Battlefield || object.kind() == ObjectKind::Ability {
            continue;
        }
        let here = working_here(object)
            .map(|(address, rules)| (address, Arc::clone(rules)))
            .chain(provided_here(object));
        object_triggers(
            state,
            object,
            zone_kind(object.zone()),
            here,
            happened,
            found,
            used,
        );
    }
}

/// How many additional times a triggered ability of a permanent `controller` controls triggers on `happened`: once for
/// each rule in force that says so and whose event this is.
fn repeats(state: &GameState, happened: &Happened, controller: PlayerId) -> usize {
    triggers_again(state)
        .iter()
        .filter(|again| again.controller == controller)
        .filter(|again| {
            let scope = Scope::announcing(state, again.controller, again.source);
            matching(&again.cause, happened, &scope).is_some()
        })
        .count()
}

/// The delayed triggered abilities of `post` that trigger on `happened`, judged in `state`.
fn delayed(
    state: &GameState,
    post: &GameState,
    happened: &Happened,
    found: &mut Vec<Triggered>,
    used: &mut Used,
) {
    for waiting in post.delayed() {
        if used.delayed.contains(&waiting.id()) || waiting.lapsed(post) {
            continue;
        }
        let Some(pattern) = waiting.pattern() else {
            continue;
        };
        let (source, controller) = (waiting.source(), waiting.controller());
        let scope = Scope::announcing(state, controller, source);
        let Some(cause) = matching(pattern, happened, &scope) else {
            continue;
        };
        let live = snapshot(state, source.id)
            .filter(|shot| shot.counter == source.counter)
            .map(|shot| Snapshot {
                x: waiting.snapshot.x,
                ..shot
            });
        if waiting.once {
            used.delayed.insert(waiting.id());
        }
        found.push(Triggered {
            source,
            ability: waiting.ability,
            origin: AbilitySource::Printed,
            controller,
            cause,
            snapshot: live.unwrap_or_else(|| waiting.snapshot.clone()),
            delayed: Some(waiting.id()),
        });
    }
}

/// Whether [`detect`] reads the state before `events`: only a move out of the battlefield, a death, a discard and a
/// spell or ability that ends without resolving look back at it. Any other batch is judged by the state after it
/// alone, which saves copying the whole state for it.
pub(crate) fn needs_before(events: &[Event]) -> bool {
    events.iter().any(|event| match event {
        Event::CreatureDied { .. } | Event::CardDiscarded { .. } => true,
        Event::ZoneChanged { from, .. } => *from == Zone::Battlefield,
        Event::ResolutionEnded { outcome, .. } => *outcome != ResolutionOutcome::Resolved,
        _ => false,
    })
}

/// The triggered abilities that trigger on `events`, in the order they triggered: event by event, a permanent's
/// abilities in the order of the battlefield, then the delayed ones.
///
/// `pre` is the state before the events and `post` the state after. A look-back pattern (leaves the battlefield,
/// dies, is sacrificed) is judged against `pre`, every other against `post`. An intervening "if" is judged with the
/// same state as the pattern, and an ability limited to once each turn does not trigger again.
pub fn detect(pre: &GameState, events: &[Event], post: &GameState) -> Vec<Triggered> {
    if !post.watches() {
        return Vec::new();
    }
    let mut found = Vec::new();
    let mut used = Used::default();
    let all = happenings(pre, events, post);
    for happened in &all {
        let state = if happened.looks_back() { pre } else { post };
        permanents(state, happened, &mut found, &mut used);
        elsewhere(state, happened, &mut found, &mut used);
        delayed(state, post, happened, &mut found, &mut used);
    }
    // The events of the batch together, after each of them: what looks back is judged against `pre`, the rest `post`.
    let (back, forward): (Vec<Happened>, Vec<Happened>) =
        all.into_iter().partition(Happened::looks_back);
    for (state, members) in [(pre, back), (post, forward)] {
        if !members.is_empty() {
            let together = Happened::Batch(members);
            permanents(state, &together, &mut found, &mut used);
            elsewhere(state, &together, &mut found, &mut used);
        }
    }
    found
}

#[cfg(test)]
mod tests {
    //! Goal: each pattern of the card DSL triggers on the event it names and on nothing else, judged against the state the rules say (before the event for what leaves the battlefield, after it for the rest), with the right controller, cause and snapshot; one event triggers an ability once however many of its patterns it fits, several events trigger it several times, a limit and an intervening "if" hold it back, and a delayed trigger fires on the next event it names.
    use std::sync::Arc;

    use mtg_cards::CardDefinition;
    use mtg_cards::cost::Cost;
    use mtg_cards::effect::Effect;
    use mtg_cards::refs::{Bounds, Ref, TargetSpec, Targetable};
    use mtg_cards::value::{Amount, Cmp, Condition};
    use mtg_core::card::CardType;
    use rstest::rstest;

    use super::*;
    use crate::abilities::PutOnStack;
    use crate::combat::AttackTarget;
    use crate::events::DeathCause;
    use crate::fixtures::{creature, state};
    use crate::fold::apply_event;
    use crate::triggers::{DelayedCreated, DelayedId};

    const ME: PlayerId = PlayerId(0);
    const FOE: PlayerId = PlayerId(1);

    fn pattern(json: &str) -> TriggerPattern {
        serde_json::from_str(json).unwrap()
    }

    fn gain() -> Vec<Effect> {
        vec![Effect::GainLife {
            who: Ref::You,
            amount: Amount::Fixed(1),
        }]
    }

    /// "When ...: gain 1 life", with the event given as the JSON of a pattern.
    fn triggered(when: &str) -> Ability {
        Ability::Triggered {
            when: pattern(when),
            intervening_if: None,
            limit: None,
            from: ZoneKind::Battlefield,
            targets: Vec::new(),
            effects: gain(),
        }
    }

    fn with(name: &str, rules: Vec<Ability>) -> Arc<CardDefinition> {
        let mut card = CardDefinition::new(name)
            .with_type(CardType::Creature)
            .with_power_toughness(1, 1);
        for rule in rules {
            card = card.with_rules(rule);
        }
        Arc::new(card)
    }

    fn instant(name: &str) -> Arc<CardDefinition> {
        Arc::new(CardDefinition::new(name).with_type(CardType::Instant))
    }

    fn land(name: &str) -> Arc<CardDefinition> {
        Arc::new(CardDefinition::new(name).with_type(CardType::Land))
    }

    /// Seat 0 holds a Spell and a Field and controls the `watcher` and a Bear; seat 1 holds a Their Spell and a Their
    /// Field and controls an Ogre.
    fn board(watcher: Arc<CardDefinition>) -> GameState {
        state(
            vec![
                vec![instant("Spell"), land("Field")],
                vec![instant("Their Spell"), land("Their Field")],
            ],
            vec![
                vec![watcher, creature("Bear", 2, 2)],
                vec![creature("Ogre", 3, 4)],
            ],
        )
    }

    fn id(state: &GameState, name: &str) -> ObjectId {
        state
            .objects()
            .find(|o| o.card().name == name)
            .map(GameObject::id)
            .unwrap()
    }

    fn reference(state: &GameState, name: &str) -> ObjectRef {
        state.object(id(state, name)).unwrap().reference()
    }

    fn fold(state: &GameState, events: &[Event]) -> GameState {
        let mut next = state.clone();
        for event in events {
            apply_event(&mut next, event).unwrap();
        }
        next
    }

    /// What `events` set off on top of `pre`, and the state after them.
    fn fire(pre: &GameState, events: &[Event]) -> (GameState, Vec<Triggered>) {
        let post = fold(pre, events);
        let found = detect(pre, events, &post);
        (post, found)
    }

    /// The sources and abilities that triggered, by name.
    fn who(state: &GameState, found: &[Triggered]) -> Vec<(String, usize)> {
        found
            .iter()
            .map(|t| {
                let name = state.object(t.source.id).unwrap().card().name.clone();
                (name, t.ability)
            })
            .collect()
    }

    fn died(state: &GameState, name: &str) -> Event {
        Event::CreatureDied {
            object: id(state, name),
            cause: DeathCause::LethalDamage,
        }
    }

    fn cast(state: &GameState, player: PlayerId, name: &str, targets: Vec<Target>) -> Event {
        Event::SpellCast {
            player,
            object: id(state, name),
            choices: Choices {
                targets: vec![targets],
                ..Choices::default()
            },
        }
    }

    #[rstest]
    #[case::damage(r#"{"DealsDamage":{"source":[]}}"#)]
    #[case::life_gained(r#"{"LifeGained":{}}"#)]
    #[case::life_lost(r#"{"LifeLost":{}}"#)]
    #[case::drawing(r#"{"Drew":{}}"#)]
    #[case::discarding(r#"{"Discarded":{}}"#)]
    #[case::becoming_tapped(r#"{"Tapped":{"what":[]}}"#)]
    #[case::becoming_untapped(r#"{"Untapped":{"what":[]}}"#)]
    #[case::exploring(r#"{"Explored":{"what":[]}}"#)]
    #[case::conniving(r#"{"Connived":{"what":[]}}"#)]
    #[case::tapping_for_mana(r#"{"TappedForMana":{"what":[]}}"#)]
    #[case::blocking(r#"{"Blocks":{"what":[]}}"#)]
    #[case::becoming_blocked(r#"{"BecomesBlocked":{"what":[]}}"#)]
    #[case::attacking_together(r#"{"GroupAttacks":{"what":[]}}"#)]
    #[case::counters_put(r#"{"CountersPut":{}}"#)]
    #[case::counters_removed(r#"{"CountersRemoved":{}}"#)]
    #[case::attaching(r#"{"Attached":{}}"#)]
    #[case::cycling(r#"{"Cycled":{}}"#)]
    #[case::becoming_the_monarch(r#"{"BecomesMonarch":{}}"#)]
    #[case::proliferating(r#"{"Proliferated":{}}"#)]
    #[case::amassing(r#"{"Amassed":{}}"#)]
    #[case::bolstering(r#"{"Bolstered":{}}"#)]
    #[case::becoming_monstrous(r#"{"BecomesMonstrous":{"what":[]}}"#)]
    #[case::being_turned_face_up(r#"{"TurnedFaceUp":{"what":[]}}"#)]
    #[case::adapting(r#"{"Adapted":{"what":[]}}"#)]
    fn the_patterns_of_events_about_players_and_permanents_trigger_on_no_other_event(
        #[case] when: &str,
    ) {
        let pre = board(with("Watcher", vec![triggered(when)]));
        let events = [
            Event::StepBegan { step: Step::Upkeep },
            Event::ManaPoolEmptied { player: ME },
            died(&pre, "Bear"),
        ];
        let (_, found) = fire(&pre, &events);
        assert_eq!(found, []);
    }

    #[test]
    fn a_game_without_triggered_abilities_finds_nothing() {
        let pre = state(
            vec![vec![], vec![]],
            vec![vec![creature("Bear", 2, 2)], vec![]],
        );
        let (_, found) = fire(&pre, &[died(&pre, "Bear")]);
        assert!(!pre.watches());
        assert_eq!(found, []);
    }

    #[test]
    fn an_enters_ability_triggers_after_the_spell_resolves_and_not_before() {
        let watcher = with(
            "Watcher",
            vec![triggered(
                r#"{"ZoneChange":{"what":["This"],"to":"Battlefield"}}"#,
            )],
        );
        let pre = state(vec![vec![watcher], vec![]], vec![vec![], vec![]]);
        let spell = id(&pre, "Watcher");
        let cast = Event::SpellCast {
            player: ME,
            object: spell,
            choices: Choices::default(),
        };
        let (on_the_stack, found) = fire(&pre, std::slice::from_ref(&cast));
        assert_eq!(found, [], "casting it is not entering the battlefield");
        let (after, found) = fire(&on_the_stack, &[Event::SpellResolved { object: spell }]);
        assert_eq!(who(&after, &found), [("Watcher".to_owned(), 0)]);
        let entered = reference(&after, "Watcher");
        assert_eq!(
            found[0].source, entered,
            "the new object, on the battlefield"
        );
        assert_eq!(found[0].controller, ME);
        assert_eq!(found[0].cause.subjects, [Target::Object(entered)]);
        assert_eq!(found[0].snapshot.zone, Zone::Battlefield);
    }

    #[test]
    fn a_dies_ability_looks_back_at_the_creature_as_it_was() {
        let watcher = with(
            "Watcher",
            vec![triggered(
                r#"{"ZoneChange":{"what":["This"],"from":"Battlefield","to":"Graveyard"}}"#,
            )],
        );
        let pre = board(watcher);
        let before = reference(&pre, "Watcher");
        let (post, found) = fire(&pre, &[died(&pre, "Watcher")]);
        assert_eq!(who(&post, &found), [("Watcher".to_owned(), 0)]);
        assert_eq!(
            found[0].source, before,
            "the object that was on the battlefield"
        );
        assert_eq!(found[0].snapshot.zone, Zone::Battlefield);
        assert_eq!(found[0].snapshot.counter, before.counter);
        assert_eq!(found[0].controller, ME);
        assert_eq!(found[0].cause.subjects, [Target::Object(before)]);
        assert_eq!(found[0].cause.snapshots[0].zone, Zone::Battlefield);
        assert_ne!(
            reference(&post, "Watcher").counter,
            before.counter,
            "it is a new object in the graveyard now"
        );
    }

    #[test]
    fn creatures_that_die_together_see_each_other_die() {
        let observer = with(
            "Observer",
            vec![triggered(
                r#"{"ZoneChange":{"what":[{"Type":"Creature"},{"ControlledBy":"You"}],"from":"Battlefield","to":"Graveyard"}}"#,
            )],
        );
        let pre = board(observer);
        // The Ogre is not the observer's; the Bear is, and so is the observer itself.
        let (post, found) = fire(&pre, &[died(&pre, "Ogre")]);
        assert_eq!(found, [], "{:?}", who(&post, &found));
        let (post, found) = fire(&pre, &[died(&pre, "Bear")]);
        assert_eq!(who(&post, &found), [("Observer".to_owned(), 0)]);
        let together = [
            died(&pre, "Observer"),
            died(&pre, "Bear"),
            died(&pre, "Ogre"),
        ];
        let (post, found) = fire(&pre, &together);
        assert_eq!(
            who(&post, &found),
            [("Observer".to_owned(), 0), ("Observer".to_owned(), 0)],
            "once for itself and once for the Bear, though it left the battlefield with them"
        );
    }

    #[rstest]
    #[case::my_land(ME, "Field", 1)]
    #[case::their_land(FOE, "Their Field", 0)]
    fn a_land_entering_triggers_the_abilities_that_watch_for_lands_of_their_controller(
        #[case] player: PlayerId,
        #[case] field: &str,
        #[case] expected: usize,
    ) {
        let landfall = with(
            "Landfall",
            vec![triggered(
                r#"{"ZoneChange":{"what":[{"Type":"Land"},{"ControlledBy":"You"}],"to":"Battlefield"}}"#,
            )],
        );
        let pre = board(landfall);
        let played = Event::LandPlayed {
            player,
            object: id(&pre, field),
            face: None,
        };
        let (post, found) = fire(&pre, &[played]);
        assert_eq!(found.len(), expected, "{:?}", who(&post, &found));
    }

    #[rstest]
    #[case::my_card(ME, "Spell", 1)]
    #[case::their_card(FOE, "Their Spell", 0)]
    fn a_discard_triggers_what_watches_for_cards_of_its_player(
        #[case] player: PlayerId,
        #[case] card: &str,
        #[case] expected: usize,
    ) {
        let pre = board(with(
            "Madness",
            vec![triggered(
                r#"{"ZoneChange":{"what":[{"OwnedBy":"You"}],"from":"Hand","to":"Graveyard"}}"#,
            )],
        ));
        let discard = Event::CardDiscarded {
            player,
            object: id(&pre, card),
        };
        assert_eq!(fire(&pre, &[discard]).1.len(), expected);
    }

    #[test]
    fn a_draw_triggers_what_watches_for_cards_leaving_a_library_for_a_hand() {
        let pre = board(with(
            "Tithe",
            vec![triggered(
                r#"{"ZoneChange":{"what":[],"from":"Library","to":"Hand"}}"#,
            )],
        ));
        let top = *pre.zone(Zone::Library(FOE)).last().unwrap();
        let draw = Event::CardDrawn {
            player: FOE,
            object: top,
        };
        let (post, found) = fire(&pre, &[draw]);
        assert_eq!(who(&post, &found), [("Tithe".to_owned(), 0)]);
        assert_eq!(
            found[0].cause.snapshots[0].zone,
            Zone::Hand(FOE),
            "the card is in a hand when the ability is photographed"
        );
    }

    /// A defender of the table below: a player, or a planeswalker (the Watcher stands in for one).
    type Defender = Option<PlayerId>;

    #[rstest]
    #[case::a_player_is_attacked(r#"{"PlayerAttacked":{"who":"EachPlayer"}}"#, &[("Bear", Some(FOE))], &[Target::Player(FOE)])]
    #[case::two_attackers_trigger_it_once(r#"{"PlayerAttacked":{"who":"EachPlayer"}}"#, &[("Bear", Some(FOE)), ("Watcher", Some(FOE))], &[Target::Player(FOE)])]
    #[case::another_player_is_attacked(r#"{"PlayerAttacked":{"who":"You"}}"#, &[("Bear", Some(FOE))], &[])]
    #[case::the_controller_is_attacked(r#"{"PlayerAttacked":{"who":"You"}}"#, &[("Ogre", Some(ME))], &[Target::Player(ME)])]
    #[case::only_a_planeswalker_is_attacked(r#"{"PlayerAttacked":{"who":"EachPlayer"}}"#, &[("Bear", None)], &[])]
    #[case::a_player_and_a_planeswalker(r#"{"PlayerAttacked":{"who":"EachPlayer"}}"#, &[("Bear", Some(FOE)), ("Watcher", None)], &[Target::Player(FOE)])]
    #[case::a_player_attacks_an_opponent_once(r#"{"PlayerAttacks":{"at":"Opponents"}}"#, &[("Bear", Some(FOE)), ("Watcher", Some(FOE))], &[Target::Player(ME)])]
    #[case::a_player_attacks_another(r#"{"PlayerAttacks":{"at":"You"}}"#, &[("Bear", Some(FOE))], &[])]
    #[case::an_opponent_attacks_the_controller(r#"{"PlayerAttacks":{"by":"Opponents","at":"You"}}"#, &[("Ogre", Some(ME))], &[Target::Player(FOE)])]
    #[case::the_controller_attacks_a_planeswalker(r#"{"PlayerAttacks":{"at":"Opponents"}}"#, &[("Bear", None)], &[])]
    fn an_attack_at_a_player_triggers_once_for_the_player_or_for_the_attacker(
        #[case] when: &str,
        #[case] attacks: &[(&str, Defender)],
        #[case] subjects: &[Target],
    ) {
        let pre = board(with("Watcher", vec![triggered(when)]));
        let by = if attacks[0].0 == "Ogre" { FOE } else { ME };
        let event = Event::AttackersDeclared {
            player: by,
            attackers: attacks
                .iter()
                .map(|(name, defender)| {
                    let at = match defender {
                        Some(player) => AttackTarget::Player(*player),
                        None => AttackTarget::Planeswalker(id(&pre, "Ogre")),
                    };
                    (id(&pre, name), at)
                })
                .collect(),
        };
        let (_, found) = fire(&pre, &[event]);
        let seen: Vec<Vec<Target>> = found.iter().map(|t| t.cause.subjects.clone()).collect();
        let expected: Vec<Vec<Target>> = if subjects.is_empty() {
            Vec::new()
        } else {
            vec![subjects.to_vec()]
        };
        assert_eq!(seen, expected);
    }

    #[test]
    fn each_attacker_triggers_what_watches_for_it() {
        let alone = with("Alone", vec![triggered(r#"{"Attacks":{"what":["This"]}}"#)]);
        let pre = board(alone);
        let attack = |names: &[&str]| Event::AttackersDeclared {
            player: ME,
            attackers: names
                .iter()
                .map(|n| (id(&pre, n), AttackTarget::Player(FOE)))
                .collect(),
        };
        let (post, found) = fire(&pre, &[attack(&["Alone", "Bear"])]);
        assert_eq!(who(&post, &found), [("Alone".to_owned(), 0)]);
        assert_eq!(
            found[0].cause.subjects,
            [Target::Object(reference(&pre, "Alone"))]
        );
        assert_eq!(fire(&pre, &[attack(&["Bear"])]).1, []);

        let all = with(
            "All",
            vec![triggered(r#"{"Attacks":{"what":[{"Type":"Creature"}]}}"#)],
        );
        let pre = board(all);
        let both = Event::AttackersDeclared {
            player: ME,
            attackers: vec![
                (id(&pre, "All"), AttackTarget::Player(FOE)),
                (id(&pre, "Bear"), AttackTarget::Player(FOE)),
            ],
        };
        let (post, found) = fire(&pre, &[both]);
        assert_eq!(
            found.len(),
            2,
            "one for each creature: {:?}",
            who(&post, &found)
        );
    }

    #[rstest]
    #[case::an_opponent_casts_and_it_watches_opponents(
        r#"{"SpellCast":{"by":"Opponents"}}"#,
        FOE,
        "Their Spell",
        1
    )]
    #[case::i_cast_and_it_watches_opponents(r#"{"SpellCast":{"by":"Opponents"}}"#, ME, "Spell", 0)]
    #[case::i_cast_and_it_watches_me(r#"{"SpellCast":{"by":"You"}}"#, ME, "Spell", 1)]
    #[case::an_opponent_casts_and_it_watches_me(
        r#"{"SpellCast":{"by":"You"}}"#,
        FOE,
        "Their Spell",
        0
    )]
    #[case::anyone_casts(r#"{"SpellCast":{}}"#, FOE, "Their Spell", 1)]
    #[case::it_watches_creatures_and_an_instant_is_cast(
        r#"{"SpellCast":{"what":[{"Type":"Creature"}]}}"#,
        ME,
        "Spell",
        0
    )]
    #[case::it_watches_instants_and_one_is_cast(
        r#"{"SpellCast":{"what":[{"Type":"Instant"}]}}"#,
        ME,
        "Spell",
        1
    )]
    fn a_spell_cast_triggers_what_watches_for_that_caster_and_that_spell(
        #[case] when: &str,
        #[case] caster: PlayerId,
        #[case] spell: &str,
        #[case] expected: usize,
    ) {
        let pre = board(with("Study", vec![triggered(when)]));
        let event = cast(&pre, caster, spell, Vec::new());
        let (post, found) = fire(&pre, &[event]);
        assert_eq!(found.len(), expected, "{:?}", who(&post, &found));
        if let Some(found) = found.first() {
            assert_eq!(
                found.cause.subjects,
                [
                    Target::Player(caster),
                    Target::Object(reference(&post, spell))
                ],
                "the caster and the spell, which is on the stack now"
            );
            assert_eq!(found.snapshot.zone, Zone::Battlefield);
        }
    }

    #[rstest]
    #[case::my_sacrifice(ZoneCause::Sacrificed, "Bear", 1)]
    #[case::a_destruction(ZoneCause::Destroyed, "Bear", 0)]
    #[case::their_sacrifice(ZoneCause::Sacrificed, "Ogre", 0)]
    fn only_a_sacrifice_by_the_right_player_triggers_a_sacrifice_ability(
        #[case] cause: ZoneCause,
        #[case] victim: &str,
        #[case] expected: usize,
    ) {
        let pre = board(with(
            "Altar",
            vec![triggered(r#"{"Sacrificed":{"what":[],"by":"You"}}"#)],
        ));
        let owner = pre.object(id(&pre, victim)).unwrap().owner();
        let moved = Event::ZoneChanged {
            object: id(&pre, victim),
            from: Zone::Battlefield,
            to: Zone::Graveyard(owner),
            cause,
        };
        let (post, found) = fire(&pre, &[moved]);
        assert_eq!(found.len(), expected, "{:?}", who(&post, &found));
        if let Some(found) = found.first() {
            assert_eq!(
                found.cause.subjects,
                [Target::Object(reference(&pre, victim)), Target::Player(ME)],
                "the permanent as it was and who sacrificed it"
            );
        }
    }

    #[rstest]
    #[case::my_upkeep_for_me(r#"{"StepBegins":{"step":"Upkeep","whose":"You"}}"#, Step::Upkeep, 1)]
    #[case::the_wrong_step(r#"{"StepBegins":{"step":"Upkeep","whose":"You"}}"#, Step::Draw, 0)]
    #[case::their_upkeep_is_not_mine(
        r#"{"StepBegins":{"step":"Upkeep","whose":"Opponents"}}"#,
        Step::Upkeep,
        0
    )]
    #[case::each_upkeep(r#"{"StepBegins":{"step":"Upkeep"}}"#, Step::Upkeep, 1)]
    #[case::the_end_step(r#"{"StepBegins":{"step":"End","whose":"You"}}"#, Step::End, 1)]
    fn a_step_beginning_triggers_what_waits_for_that_step_of_that_player(
        #[case] when: &str,
        #[case] step: Step,
        #[case] expected: usize,
    ) {
        // Turn 1 is seat 0's.
        let pre = board(with("Clock", vec![triggered(when)]));
        let (post, found) = fire(&pre, &[Event::StepBegan { step }]);
        assert_eq!(found.len(), expected, "{:?}", who(&post, &found));
        if let Some(found) = found.first() {
            assert_eq!(found.cause.subjects, [Target::Player(ME)]);
        }
    }

    #[rstest]
    #[case::every_players_step_still_begins(r#"{"StepBegins":{"step":"End"}}"#, 1)]
    #[case::my_step_is_not_theirs(r#"{"StepBegins":{"step":"End","whose":"You"}}"#, 0)]
    #[case::an_opponents_step_has_no_opponent(
        r#"{"StepBegins":{"step":"End","whose":"Opponents"}}"#,
        0
    )]
    fn a_step_of_a_turn_without_an_active_player_triggers_only_what_names_no_player(
        #[case] when: &str,
        #[case] expected: usize,
    ) {
        let mut pre = board(with("Clock", vec![triggered(when)]));
        let turn = Event::TurnBegan {
            number: 2,
            active: FOE,
        };
        let lost = Event::PlayerLost {
            player: FOE,
            reason: crate::player::LossReason::Conceded,
        };
        apply_event(&mut pre, &turn).unwrap();
        apply_event(&mut pre, &lost).unwrap();
        let (post, found) = fire(&pre, &[Event::StepBegan { step: Step::End }]);
        assert_eq!(found.len(), expected, "{:?}", who(&post, &found));
        if let Some(found) = found.first() {
            assert_eq!(found.cause.subjects, [], "no player's step");
        }
    }

    #[test]
    fn a_step_of_another_players_turn_triggers_the_abilities_that_watch_every_players() {
        let each = with(
            "Each",
            vec![triggered(r#"{"StepBegins":{"step":"Upkeep"}}"#)],
        );
        let mut pre = board(each);
        let turn = Event::TurnBegan {
            number: 2,
            active: FOE,
        };
        apply_event(&mut pre, &turn).unwrap();
        let (post, found) = fire(&pre, &[Event::StepBegan { step: Step::Upkeep }]);
        assert_eq!(who(&post, &found), [("Each".to_owned(), 0)]);
        assert_eq!(
            found[0].cause.subjects,
            [Target::Player(FOE)],
            "that player's upkeep"
        );
    }

    #[test]
    fn becoming_the_target_of_a_spell_triggers_the_object_and_names_what_targets_it_and_who_controls_that()
     {
        let guarded = with(
            "Guarded",
            vec![triggered(r#"{"BecomesTarget":{"what":["This"]}}"#)],
        );
        let pre = board(guarded);
        let at = |name: &str| Target::Object(reference(&pre, name));
        let (post, found) = fire(&pre, &[cast(&pre, ME, "Spell", vec![at("Guarded")])]);
        assert_eq!(who(&post, &found), [("Guarded".to_owned(), 0)]);
        assert_eq!(
            found[0].cause.subjects,
            [
                at("Guarded"),
                Target::Object(reference(&post, "Spell")),
                Target::Player(ME)
            ],
            "what was targeted, what targets it and who controls that"
        );
        assert_eq!(
            fire(&pre, &[cast(&pre, ME, "Spell", vec![at("Ogre")])]).1,
            []
        );
        let at_a_player = cast(&pre, ME, "Spell", vec![Target::Player(FOE)]);
        assert_eq!(
            fire(&pre, &[at_a_player]).1,
            [],
            "a player is not an object"
        );
    }

    #[test]
    fn becoming_the_target_of_an_ability_triggers_it_too() {
        let pinger = Arc::new(
            CardDefinition::new("Pinger")
                .with_type(CardType::Creature)
                .with_power_toughness(1, 1)
                .with_rules(Ability::Activated {
                    cost: Cost(Vec::new()),
                    timing: mtg_cards::ability::Timing::Instant,
                    condition: None,
                    limit: None,
                    from: ZoneKind::Battlefield,
                    targets: vec![TargetSpec {
                        what: Targetable::AnyTarget,
                        bounds: Bounds::default(),
                        divide: None,
                    }],
                    effects: vec![Effect::DealDamage {
                        source: None,
                        to: Ref::Target(0),
                        amount: Amount::Fixed(1),
                    }],
                }),
        );
        let guarded = with(
            "Guarded",
            vec![triggered(
                r#"{"BecomesTarget":{"what":["This"],"by":[{"Not":"Spell"}]}}"#,
            )],
        );
        let pre = state(vec![vec![], vec![]], vec![vec![pinger, guarded], vec![]]);
        let put = Event::AbilityPutOnStack(Box::new(PutOnStack {
            origin: crate::abilities::AbilitySource::default(),
            object: pre.next_object(),
            source: reference(&pre, "Pinger"),
            snapshot: snapshot(&pre, id(&pre, "Pinger")).unwrap(),
            ability: 0,
            controller: ME,
            choices: Choices {
                targets: vec![vec![Target::Object(reference(&pre, "Guarded"))]],
                ..Choices::default()
            },
            cause: None,
            delayed: None,
        }));
        let (post, found) = fire(&pre, &[put]);
        assert_eq!(who(&post, &found), [("Guarded".to_owned(), 0)]);
    }

    #[test]
    fn an_event_that_fits_two_patterns_of_an_ability_triggers_it_once() {
        let either = with(
            "Either",
            vec![triggered(
                r#"{"AnyOf":[{"ZoneChange":{"what":["This"],"from":"Battlefield","to":"Graveyard"}},{"ZoneChange":{"what":["This"],"from":"Battlefield"}}]}"#,
            )],
        );
        let pre = board(either);
        let (post, found) = fire(&pre, &[died(&pre, "Either")]);
        assert_eq!(who(&post, &found), [("Either".to_owned(), 0)]);
    }

    #[test]
    fn several_abilities_and_several_events_each_trigger_in_the_order_they_happened() {
        let on_death = r#"{"ZoneChange":{"what":[{"Type":"Creature"}],"from":"Battlefield","to":"Graveyard"}}"#;
        let two = with("Two", vec![triggered(on_death), triggered(on_death)]);
        let pre = board(two);
        let (post, found) = fire(&pre, &[died(&pre, "Bear"), died(&pre, "Ogre")]);
        assert_eq!(
            who(&post, &found),
            [
                ("Two".to_owned(), 0),
                ("Two".to_owned(), 1),
                ("Two".to_owned(), 0),
                ("Two".to_owned(), 1),
            ],
            "the Bear's death first, ability by ability, then the Ogre's"
        );
    }

    #[test]
    fn what_is_limited_to_once_a_turn_triggers_once_in_a_batch_and_not_again_until_the_turn_ends() {
        let mut limited =
            triggered(r#"{"ZoneChange":{"what":[{"Type":"Land"}],"to":"Battlefield"}}"#);
        if let Ability::Triggered { limit, .. } = &mut limited {
            *limit = Some(Limit::OncePerTurn);
        }
        let pre = board(with("Once", vec![limited]));
        let lands = [
            Event::LandPlayed {
                player: ME,
                object: id(&pre, "Field"),
                face: None,
            },
            Event::LandPlayed {
                player: FOE,
                object: id(&pre, "Their Field"),
                face: None,
            },
        ];
        let (post, found) = fire(&pre, &lands);
        assert_eq!(found.len(), 1, "two lands in one batch, one trigger");
        // Once the trigger is in the state, the limit is spent for the turn.
        let spent = fold(
            &post,
            &[Event::AbilityTriggered(Box::new(found[0].clone()))],
        );
        assert!(spent.limit_spent(found[0].source, found[0].origin, found[0].ability));
        let next = fold(
            &spent,
            &[Event::TurnBegan {
                number: 2,
                active: FOE,
            }],
        );
        assert!(
            !next.limit_spent(found[0].source, found[0].origin, found[0].ability),
            "the new turn gives it back"
        );
    }

    #[rstest]
    #[case::the_condition_holds(20, 1)]
    #[case::the_condition_does_not_hold(5, 0)]
    fn an_intervening_if_is_checked_as_the_event_happens(
        #[case] life: i32,
        #[case] expected: usize,
    ) {
        let mut conditional = triggered(r#"{"StepBegins":{"step":"Upkeep","whose":"You"}}"#);
        if let Ability::Triggered { intervening_if, .. } = &mut conditional {
            *intervening_if = Some(Condition::Compare {
                left: Amount::Life(Ref::You),
                op: Cmp::Ge,
                right: Amount::Fixed(10),
            });
        }
        let mut pre = board(with("Felidar", vec![conditional]));
        if life != 20 {
            let lose = Event::LifeLost {
                player: ME,
                amount: u32::try_from(20 - life).unwrap(),
            };
            apply_event(&mut pre, &lose).unwrap();
        }
        let (post, found) = fire(&pre, &[Event::StepBegan { step: Step::Upkeep }]);
        assert_eq!(found.len(), expected, "{:?}", who(&post, &found));
    }

    fn delayed_end_step(once: bool, state: &GameState) -> Event {
        Event::DelayedTriggerCreated(Box::new(DelayedCreated {
            id: DelayedId(0),
            source: reference(state, "Bear"),
            snapshot: snapshot(state, id(state, "Bear")).unwrap(),
            controller: ME,
            ability: 3,
            when: pattern(r#"{"StepBegins":{"step":"End"}}"#),
            once,
            effects: gain(),
            bindings: vec![("kept".to_owned(), vec![Target::Player(FOE)])],
        }))
    }

    #[test]
    fn a_delayed_trigger_fires_on_the_next_event_it_names_and_once_only() {
        let pre = board(with("Quiet", Vec::new()));
        let waiting = fold(&pre, &[delayed_end_step(true, &pre)]);
        assert_eq!(waiting.delayed().len(), 1);
        let (_, found) = fire(&waiting, &[Event::StepBegan { step: Step::Draw }]);
        assert_eq!(found, [], "not the step it waits for");
        let (post, found) = fire(&waiting, &[Event::StepBegan { step: Step::End }]);
        assert_eq!(found.len(), 1);
        let t = &found[0];
        assert_eq!(
            (t.delayed, t.ability, t.controller),
            (Some(DelayedId(0)), 3, ME)
        );
        assert_eq!(t.source, reference(&post, "Bear"));
        let fired = fold(&post, &[Event::AbilityTriggered(Box::new(t.clone()))]);
        assert_eq!(fired.delayed().len(), 0, "one that fires once is gone");
        let queued = &fired.pending_triggers()[0];
        assert_eq!(
            queued.bindings.get("kept"),
            Some(&vec![Target::Player(FOE)]),
            "it took the names the resolution had bound"
        );
        // Two end steps in one batch fire it once.
        let both = [
            Event::StepBegan { step: Step::End },
            Event::StepBegan { step: Step::End },
        ];
        assert_eq!(fire(&waiting, &both).1.len(), 1);
    }

    #[test]
    fn a_delayed_trigger_that_is_not_once_lasts_the_turn() {
        let pre = board(with("Quiet", Vec::new()));
        let waiting = fold(&pre, &[delayed_end_step(false, &pre)]);
        let end = Event::StepBegan { step: Step::End };
        let (post, found) = fire(&waiting, std::slice::from_ref(&end));
        assert_eq!(found.len(), 1);
        let fired = fold(
            &post,
            &[Event::AbilityTriggered(Box::new(found[0].clone()))],
        );
        assert_eq!(
            fired.delayed().len(),
            1,
            "it stays for the rest of the turn"
        );
        let later = fold(
            &fired,
            &[Event::TurnBegan {
                number: 2,
                active: FOE,
            }],
        );
        assert!(
            later.delayed()[0].lapsed(&later),
            "and runs out when the turn does"
        );
        assert_eq!(
            fire(&later, std::slice::from_ref(&end)).1,
            [],
            "so it no longer fires"
        );
    }

    #[rstest]
    #[case::a_death(vec![Event::CreatureDied { object: ObjectId(1), cause: DeathCause::LethalDamage }], true)]
    #[case::a_discard(vec![Event::CardDiscarded { player: ME, object: ObjectId(1) }], true)]
    #[case::a_move_out_of_the_battlefield(vec![Event::ZoneChanged {
        object: ObjectId(1),
        from: Zone::Battlefield,
        to: Zone::Exile,
        cause: ZoneCause::Exiled,
    }], true)]
    #[case::a_spell_that_fizzles(vec![Event::ResolutionEnded { object: ObjectId(1), outcome: ResolutionOutcome::Fizzled }], true)]
    #[case::a_spell_that_is_countered(vec![Event::ResolutionEnded { object: ObjectId(1), outcome: ResolutionOutcome::Countered }], true)]
    #[case::an_ability_whose_condition_lapsed(vec![Event::ResolutionEnded { object: ObjectId(1), outcome: ResolutionOutcome::Lapsed }], true)]
    #[case::one_of_several(vec![
        Event::StepBegan { step: Step::End },
        Event::CreatureDied { object: ObjectId(1), cause: DeathCause::LethalDamage },
    ], true)]
    #[case::a_spell_that_resolves(vec![Event::ResolutionEnded { object: ObjectId(1), outcome: ResolutionOutcome::Resolved }], false)]
    #[case::a_move_into_the_battlefield(vec![Event::ZoneChanged {
        object: ObjectId(1),
        from: Zone::Hand(ME),
        to: Zone::Battlefield,
        cause: ZoneCause::Moved,
    }], false)]
    #[case::a_land_played(vec![Event::LandPlayed { player: ME, object: ObjectId(1), face: None }], false)]
    #[case::a_draw(vec![Event::CardDrawn { player: ME, object: ObjectId(1) }], false)]
    #[case::a_spell_cast(vec![Event::SpellCast { player: ME, object: ObjectId(1), choices: Choices::default() }], false)]
    #[case::a_step(vec![Event::StepBegan { step: Step::Upkeep }], false)]
    #[case::nothing(Vec::new(), false)]
    fn only_a_batch_that_looks_back_needs_the_state_before_it(
        #[case] events: Vec<Event>,
        #[case] needed: bool,
    ) {
        assert_eq!(needs_before(&events), needed);
    }

    #[test]
    fn a_batch_that_does_not_look_back_finds_the_same_in_the_state_after_it_alone() {
        let everything = with(
            "Everything",
            [
                r#"{"ZoneChange":{"what":[]}}"#,
                r#"{"Attacks":{"what":[]}}"#,
                r#"{"SpellCast":{}}"#,
                r#"{"BecomesTarget":{"what":[]}}"#,
                r#"{"StepBegins":{"step":"Upkeep"}}"#,
            ]
            .map(triggered)
            .to_vec(),
        );
        let pre = board(everything);
        let at = Target::Object(reference(&pre, "Ogre"));
        let batches = [
            vec![Event::LandPlayed {
                player: ME,
                object: id(&pre, "Field"),
                face: None,
            }],
            vec![Event::CardDrawn {
                player: FOE,
                object: *pre.zone(Zone::Library(FOE)).last().unwrap(),
            }],
            vec![Event::AttackersDeclared {
                player: ME,
                attackers: vec![(id(&pre, "Bear"), AttackTarget::Player(FOE))],
            }],
            vec![cast(&pre, ME, "Spell", vec![at])],
            vec![Event::StepBegan { step: Step::Upkeep }],
            vec![Event::ZoneChanged {
                object: id(&pre, "Spell"),
                from: Zone::Hand(ME),
                to: Zone::Exile,
                cause: ZoneCause::Exiled,
            }],
        ];
        for events in batches {
            assert!(!needs_before(&events), "{events:?}");
            let (post, found) = fire(&pre, &events);
            assert!(!found.is_empty(), "the watcher sees {events:?}");
            assert_eq!(detect(&post, &events, &post), found, "{events:?}");
        }
    }

    #[test]
    fn a_death_judged_by_the_state_after_it_alone_would_be_missed() {
        let watcher = with(
            "Watcher",
            vec![triggered(
                r#"{"ZoneChange":{"what":["This"],"from":"Battlefield","to":"Graveyard"}}"#,
            )],
        );
        let pre = board(watcher);
        let events = [died(&pre, "Watcher")];
        assert!(needs_before(&events));
        let (post, found) = fire(&pre, &events);
        assert_eq!(found.len(), 1);
        assert_eq!(detect(&post, &events, &post), [], "it is gone by then");
    }
}
