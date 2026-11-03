//! Costs that act on the source alone: putting counters on it ("Put a -1/-1 counter on this creature") and exerting it
//! (CR 701.43).

use mtg_cards::value::CounterKind;
use mtg_core::ids::{ObjectId, PlayerId};

use super::Payer;
use crate::choose::Target;
use crate::counters::CountersChanged;
use crate::events::Event;
use crate::state::GameState;

/// Whether the source is a permanent on the battlefield, the only thing counters go on and the only thing exerted.
pub(super) fn payable(payer: &Payer<'_>) -> bool {
    payer
        .state
        .object(payer.source)
        .is_some_and(|object| object.permanent().is_some())
}

/// The counters put on the source; putting none is no event.
pub(super) fn put(
    state: &GameState,
    source: ObjectId,
    counter: &CounterKind,
    count: u32,
) -> Vec<Event> {
    let (Some(object), Ok(added)) = (state.object(source), i32::try_from(count)) else {
        return Vec::new();
    };
    if added == 0 {
        return Vec::new();
    }
    vec![Event::CountersChanged(CountersChanged {
        on: Target::Object(object.reference()),
        counter: counter.clone(),
        delta: added,
    })]
}

/// The source is exerted by the player who pays.
pub(super) fn exert(player: PlayerId, source: ObjectId) -> Vec<Event> {
    vec![Event::Exerted {
        object: source,
        player,
    }]
}
