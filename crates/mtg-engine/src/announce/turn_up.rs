//! Turning a face-down permanent face up (CR 702.37e, 702.168): a special action that does not use the stack. The
//! announcement picks what the parts of its morph, megamorph or disguise cost take ("Morph—Discard a card"), one part at
//! a time as for an ability's cost, then pays the mana, and can be backed out of until it is paid; the events that follow
//! the payment turn the permanent face up, and a megamorph gets its +1/+1 counter (CR 702.37b).

use mtg_cards::ability::Ability;
use mtg_cards::modify::Modification;
use mtg_cards::refs::Selector;
use mtg_cards::trigger::{ReplaceBy, ReplaceWhen};
use mtg_cards::value::CounterKind;
use mtg_core::ids::{ObjectId, PlayerId};
use mtg_core::mana::ManaSymbol;
use std::sync::Arc;

use super::{Done, Next, Phase, suspend};
use crate::characteristics::controller_of;
use crate::choose::{CardsOptions, Choices, NumberOptions, Target};
use crate::copies::{BecameCopy, copiable_values, may_be_copied};
use crate::counters::CountersChanged;
use crate::decision::{Decision, DecisionKind};
use crate::events::Event;
use crate::face_down::{TurnUp, turn_up};
use crate::parts::{Part, Payer};
use crate::state::GameState;

/// The next decision of turning `card` face up (a pick for a part of the cost, then the payment), or nothing when
/// `player` cannot turn it up now.
pub(super) fn next(
    state: &GameState,
    player: PlayerId,
    card: ObjectId,
    choices: &mut Choices,
    done: &mut Done,
) -> Next {
    let Some(up) = turn_up(state, card) else {
        return Next::Drop;
    };
    if controller_of(state, card) != Some(player) {
        return Next::Drop;
    }
    // "Morph {X}{X}{R}": X is announced first (CR 601.2b by way of 702.37e), and the cost takes X mana per `{X}`.
    if up.x > 0 && !done.x {
        done.asked = Some(Phase::X);
        return Next::Ask(Decision {
            player,
            kind: DecisionKind::ChooseNumber(NumberOptions {
                source: card,
                min: 0,
                max: None,
                except: Vec::new(),
                text: "Choose a value for X.".to_owned(),
                can_cancel: true,
                cost: Some(up.mana.clone()),
                per_x: up.x,
                secret: false,
            }),
        });
    }
    let mut settled = 0;
    for part in &up.parts {
        let Part::Pick(pick) = part else { continue };
        if settled < choices.cost_picks().len() {
            settled += 1;
            continue;
        }
        // The permanent being turned up never pays for itself.
        let mut barred: Vec<ObjectId> = choices.cost_picks().iter().flatten().copied().collect();
        barred.push(card);
        let candidates = pick.candidates(&Payer {
            state,
            player,
            source: card,
            barred: &barred,
            granter: None,
            pays: crate::r4707_pay_costs::PaysFor::Effect,
        });
        if pick.short(state, &candidates) {
            return Next::Drop;
        }
        if let Some(taken) = pick.unasked(state, &candidates) {
            choices.pick_for_cost(taken);
            settled += 1;
            continue;
        }
        let (min, max) = pick.bounds(candidates.len());
        done.asked = Some(Phase::Parts);
        return Next::Ask(Decision {
            player,
            kind: DecisionKind::ChooseCards(CardsOptions {
                one_per_name: false,
                source: card,
                from: candidates,
                min,
                max,
                text: pick.text(),
                can_cancel: true,
                total: None,
                groups: Vec::new(),
            }),
        });
    }
    if !payable(state, player, card, &up) {
        return Next::Drop;
    }
    // "As this Aura is turned face up, you may attach it to a creature" (CR 708.8): asked once the parts are picked,
    // among the permanents it could enchant face up with those parts paid.
    if settled == choices.cost_picks().len()
        && let Some(hosts) = r6702_hosts(state, player, card, choices)
    {
        let text = "You may choose a permanent to attach it to.";
        return optional_pick(state, (player, card), choices, done, (hosts, text));
    }
    // "As this creature enters or is turned face up, you may choose another creature" (CR 708.8): asked last.
    let attach = usize::from(r6702_may_attach(state, card).is_some());
    if choices.cost_picks().len() == settled + attach
        && let Some((options, _)) = r115_copy_options(state, player, card)
    {
        let text = "You may choose another creature to copy.";
        return optional_pick(state, (player, card), choices, done, (options, text));
    }
    let paid = choices.clone();
    let spend = crate::spend::Spend::special(mtg_cards::spend::SpecialAction::TurnFaceUp)
        .r111_by(state, player);
    let mana = up.mana.with(ManaSymbol::Generic(
        up.x.saturating_mul(choices.x.unwrap_or(0)),
    ));
    suspend::pay_for(state, player, card, (mana, spend), || {
        events(state, player, card, &paid)
    })
}

/// A pick of at most one of `options` after the parts of the cost, skipped as an empty pick when there is none.
fn optional_pick(
    state: &GameState,
    (player, card): (PlayerId, ObjectId),
    choices: &mut Choices,
    done: &mut Done,
    (options, text): (Vec<ObjectId>, &str),
) -> Next {
    if options.is_empty() {
        choices.pick_for_cost(Vec::new());
        return next(state, player, card, choices, done);
    }
    done.asked = Some(Phase::Parts);
    Next::Ask(Decision {
        player,
        kind: DecisionKind::ChooseCards(CardsOptions {
            one_per_name: false,
            source: card,
            from: options,
            min: 0,
            max: 1,
            text: text.to_owned(),
            can_cancel: false,
            total: None,
            groups: Vec::new(),
        }),
    })
}

/// Whether the parts that take no pick can be paid: the life, the counters and the cards they take are there.
fn payable(state: &GameState, player: PlayerId, card: ObjectId, up: &TurnUp) -> bool {
    let barred = [card];
    let owes = Payer {
        state,
        player,
        source: card,
        barred: &barred,
        granter: None,
        pays: crate::r4707_pay_costs::PaysFor::Effect,
    };
    up.parts
        .iter()
        .filter(|part| !matches!(part, Part::Pick(_)))
        .all(|part| part.payable(&owes, &[]).is_some())
}

/// The events of turning `card` face up: those of the parts of the cost with the objects picked for them, then the turn
/// up itself. They come between the life and the mana of the payment.
pub(crate) fn events(
    state: &GameState,
    player: PlayerId,
    card: ObjectId,
    choices: &Choices,
) -> Option<Vec<Event>> {
    let up = turn_up(state, card)?;
    let object = state.object(card)?;
    let mut events = Vec::new();
    let mut picks = choices.cost_picks().iter();
    for part in &up.parts {
        let picked = if matches!(part, Part::Pick(_)) {
            picks.next()?.as_slice()
        } else {
            &[]
        };
        events.extend(part.events(state, player, card, picked));
    }
    events.push(Event::TurnedFaceUp {
        object: card,
        player: controller_of(state, card)?,
        x: if up.x > 0 { choices.x.unwrap_or(0) } else { 0 },
    });
    if up.megamorph {
        events.push(Event::CountersChanged(CountersChanged {
            on: Target::Object(object.reference()),
            counter: CounterKind::PlusOnePlusOne,
            delta: 1,
        }));
    }
    events.extend(turned_up_counters(state, card));
    if r6702_may_attach(state, card).is_some()
        && let Some(&host) = picks.next().and_then(|picked| picked.first())
    {
        events.push(Event::Attached {
            object: card,
            to: host,
        });
    }
    if let Some(&copied) = picks.next().and_then(|picked| picked.first()) {
        events.push(r115_turned_up_copy(state, player, card, copied)?);
    }
    Some(events)
}

/// The creatures `card` may become a copy of as it is turned face up, with the exceptions of that copy: what its face-up
/// card's "As this creature enters or is turned face up, you may choose another creature" names (`ReplaceBy::AsCopy` with
/// `r115_turned_up`), judged as the turn begins. `None` when the card has no such ability.
pub(crate) fn r115_copy_options(
    state: &GameState,
    player: PlayerId,
    card: ObjectId,
) -> Option<(Vec<ObjectId>, Vec<Modification>)> {
    let object = state.object(card)?;
    let (of, except) =
        crate::ability_reader::printed_rules(&object.def).find_map(|rules| match &**rules {
            Ability::Replacement {
                by:
                    ReplaceBy::AsCopy {
                        of,
                        except,
                        r115_turned_up: true,
                        ..
                    },
                ..
            } => Some((of.clone(), except.clone())),
            _ => None,
        })?;
    let scope = crate::resolve::Scope::announcing(state, player, object.reference());
    let options = scope
        .matching(&of)
        .unwrap_or_default()
        .into_iter()
        .map(|named| named.id)
        .filter(|&id| id != card && state.object(id).is_some_and(may_be_copied))
        .collect();
    Some((options, except))
}

/// `card` becomes a copy of `copied` with the exceptions of its turned-up copy ability, until it is turned face down.
pub(crate) fn r115_turned_up_copy(
    state: &GameState,
    player: PlayerId,
    card: ObjectId,
    copied: ObjectId,
) -> Option<Event> {
    let (options, except) = r115_copy_options(state, player, card)?;
    if !options.contains(&copied) {
        return None;
    }
    let of = state.object(copied)?.reference();
    let mut values = copiable_values(state, of, None)?;
    except
        .iter()
        .all(|modification| values.except(modification))
        .then_some(())?;
    Some(Event::BecameCopy(BecameCopy {
        object: card,
        of,
        definition: Arc::new(values),
        until_end_of_turn: false,
        until_next_turn_of: None,
        attached_by: None,
        r6104_while_tapped: None,
        r707_static: None,
        r115_until_face_down: true,
    }))
}

/// The "you may attach it to" selector of the face-up card (`ReplaceBy::R6702MayAttach`), if it has one.
pub(crate) fn r6702_may_attach(state: &GameState, card: ObjectId) -> Option<Selector> {
    let object = state.object(card)?;
    crate::ability_reader::printed_rules(&object.def).find_map(|rules| match &**rules {
        Ability::Replacement {
            when: ReplaceWhen::TurnedFaceUp { what },
            by: ReplaceBy::R6702MayAttach(to),
            ..
        } if *what == Selector::this() => Some(to.clone()),
        _ => None,
    })
}

/// The permanents `card` may be attached to as it is turned face up: those its selector names that it could enchant
/// once it is face up and the parts of its cost picked in `choices` are paid (CR 303.4, 708.8). `None` when the card
/// has no such ability; the parts are judged in a copy of the state, so the sacrificed creature is never offered.
pub(crate) fn r6702_hosts(
    state: &GameState,
    player: PlayerId,
    card: ObjectId,
    choices: &Choices,
) -> Option<Vec<ObjectId>> {
    let to = r6702_may_attach(state, card)?;
    let mut after = state.clone();
    let up = turn_up(state, card)?;
    let mut picks = choices.cost_picks().iter();
    let mut events = Vec::new();
    for part in &up.parts {
        let picked = if matches!(part, Part::Pick(_)) {
            picks.next().map_or(&[][..], Vec::as_slice)
        } else {
            &[]
        };
        events.extend(part.events(state, player, card, picked));
    }
    events.push(Event::TurnedFaceUp {
        object: card,
        player,
        x: 0,
    });
    for event in &events {
        crate::fold::apply_event(&mut after, event).ok()?;
    }
    Some(r6702_hosts_in(&after, player, card, &to))
}

/// The permanents of `to` that the face-up Aura `card` could enchant in `state`.
pub(crate) fn r6702_hosts_in(
    state: &GameState,
    player: PlayerId,
    card: ObjectId,
    to: &Selector,
) -> Vec<ObjectId> {
    let Some(object) = state.object(card) else {
        return Vec::new();
    };
    let scope = crate::resolve::Scope::announcing(state, player, object.reference());
    scope
        .matching(to)
        .unwrap_or_default()
        .into_iter()
        .map(|host| host.id)
        .filter(|&host| crate::attach::legal_host(state, card, host))
        .collect()
}

/// The counters `card` gets as it is turned face up, from the "As ~ is turned face up, put N counters on it" abilities
/// of its face-up card (`ReplaceWhen::TurnedFaceUp`, CR 708.8), each counted as it turns. An amount the game cannot
/// count, or none, puts nothing.
pub(crate) fn turned_up_counters(state: &GameState, card: ObjectId) -> Vec<Event> {
    let Some(object) = state.object(card) else {
        return Vec::new();
    };
    let Some(controller) = controller_of(state, card) else {
        return Vec::new();
    };
    let scope = crate::resolve::Scope::announcing(state, controller, object.reference());
    // The card as printed: while it is face down, `card` is the face-down 2/2 it shows.
    crate::ability_reader::printed_rules(&object.def)
        .filter_map(|rules| match &**rules {
            Ability::Replacement {
                when: ReplaceWhen::TurnedFaceUp { what },
                by:
                    ReplaceBy::WithCounters {
                        counter,
                        count,
                        tapped: false,
                    },
                ..
            } if *what == Selector::this() => {
                let delta = scope.amount(count).ok().filter(|n| *n > 0)?;
                Some(Event::CountersChanged(CountersChanged {
                    on: Target::Object(object.reference()),
                    counter: counter.clone(),
                    delta: i32::try_from(delta).ok()?,
                }))
            }
            _ => None,
        })
        .collect()
}
