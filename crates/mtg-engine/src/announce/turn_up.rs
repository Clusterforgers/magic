//! Turning a face-down permanent face up (CR 702.37e, 702.168): a special action that does not use the stack. The
//! announcement is only the payment of its morph, megamorph or disguise cost, which can be backed out of; the events that
//! follow it turn the permanent face up, and a megamorph gets its +1/+1 counter (CR 702.37b).

use mtg_cards::value::CounterKind;
use mtg_core::ids::{ObjectId, PlayerId};

use super::{Next, suspend};
use crate::characteristics::controller_of;
use crate::choose::Target;
use crate::counters::CountersChanged;
use crate::events::Event;
use crate::face_down::turn_up_cost;
use crate::state::GameState;

/// The payment decision of turning `card` face up, or nothing when `player` cannot turn it up now.
pub(super) fn next(state: &GameState, player: PlayerId, card: ObjectId) -> Next {
    let Some((cost, _)) = turn_up_cost(state, card) else {
        return Next::Drop;
    };
    if events(state, card).is_none() || controller_of(state, card) != Some(player) {
        return Next::Drop;
    }
    suspend::pay(state, player, card, cost, || events(state, card))
}

/// The events of turning `card` face up. They come between the life and the mana of the payment.
pub(crate) fn events(state: &GameState, card: ObjectId) -> Option<Vec<Event>> {
    let (_, megamorph) = turn_up_cost(state, card)?;
    let object = state.object(card)?;
    let mut events = vec![Event::TurnedFaceUp {
        object: card,
        player: controller_of(state, card)?,
    }];
    if megamorph {
        events.push(Event::CountersChanged(CountersChanged {
            on: Target::Object(object.reference()),
            counter: CounterKind::PlusOnePlusOne,
            delta: 1,
        }));
    }
    Some(events)
}
