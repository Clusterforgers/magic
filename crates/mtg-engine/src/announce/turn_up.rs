//! Turning a face-down permanent face up (CR 702.37e, 702.168): a special action that does not use the stack. The
//! announcement picks what the parts of its morph, megamorph or disguise cost take ("Morph—Discard a card"), one part at
//! a time as for an ability's cost, then pays the mana, and can be backed out of until it is paid; the events that follow
//! the payment turn the permanent face up, and a megamorph gets its +1/+1 counter (CR 702.37b).

use mtg_cards::value::CounterKind;
use mtg_core::ids::{ObjectId, PlayerId};

use super::{Done, Next, Phase, suspend};
use crate::characteristics::controller_of;
use crate::choose::{CardsOptions, Choices, Target};
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
            }),
        });
    }
    if !payable(state, player, card, &up) {
        return Next::Drop;
    }
    let paid = choices.clone();
    suspend::pay(state, player, card, up.mana, || {
        events(state, player, card, &paid)
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
    });
    if up.megamorph {
        events.push(Event::CountersChanged(CountersChanged {
            on: Target::Object(object.reference()),
            counter: CounterKind::PlusOnePlusOne,
            delta: 1,
        }));
    }
    Some(events)
}
