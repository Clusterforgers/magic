//! Stun counters (CR 122.1g): a permanent with one that would become untapped loses a stun counter instead.

use mtg_cards::value::CounterKind;

use crate::choose::Target;
use crate::counters::CountersChanged;
use crate::events::Event;
use crate::object::GameObject;

/// The kind of counter a stun counter is.
pub(crate) fn kind() -> CounterKind {
    CounterKind::Other("stun".to_owned())
}

/// The event by which `object` becomes untapped: it loses a stun counter in its place when it holds one, and stays
/// tapped.
pub(crate) fn untap_event(object: &GameObject) -> Event {
    let stunned = object
        .permanent()
        .is_some_and(|permanent| permanent.counters_of(&kind()) > 0);
    if stunned {
        Event::CountersChanged(CountersChanged {
            on: Target::Object(object.reference()),
            counter: kind(),
            delta: -1,
        })
    } else {
        Event::PermanentUntapped {
            object: object.id(),
        }
    }
}
