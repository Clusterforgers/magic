//! Blight as a cost: the payer puts `N` -1/-1 counters on a creature they control and pick.

use mtg_cards::value::CounterKind;
use mtg_core::ids::ObjectId;

use crate::choose::Target;
use crate::counters::CountersChanged;
use crate::events::Event;
use crate::state::GameState;

/// The counters go on the creature picked.
pub(super) fn events(state: &GameState, object: ObjectId, count: u32) -> Vec<Event> {
    let (Some(found), Ok(delta)) = (state.object(object), i32::try_from(count)) else {
        return Vec::new();
    };
    if delta == 0 {
        return Vec::new();
    }
    vec![Event::CountersChanged(CountersChanged {
        on: Target::Object(found.reference()),
        counter: CounterKind::MinusOneMinusOne,
        delta,
    })]
}

/// "Blight 2: put 2 -1/-1 counters on a creature you control."
pub(super) fn text(count: u32) -> String {
    let counters = if count == 1 {
        "a -1/-1 counter".to_owned()
    } else {
        format!("{count} -1/-1 counters")
    };
    format!("Blight {count}: put {counters} on a creature you control.")
}
