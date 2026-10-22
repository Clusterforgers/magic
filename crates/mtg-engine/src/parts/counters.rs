//! Removing counters from the source of the ability as a cost ("Remove a charge counter from this land").

use mtg_cards::value::CounterKind;
use mtg_core::ids::ObjectId;

use super::Payer;
use crate::choose::Target;
use crate::counters::CountersChanged;
use crate::events::Event;
use crate::state::GameState;

/// The counters of the kind on the source, which is on the battlefield.
pub(super) fn most(payer: &Payer<'_>, counter: &CounterKind) -> u32 {
    payer
        .state
        .object(payer.source)
        .and_then(|object| object.permanent())
        .map_or(0, |permanent| permanent.counters_of(counter))
}

/// Whether the source is on the battlefield with at least that many counters of the kind.
pub(super) fn payable(payer: &Payer<'_>, counter: &CounterKind, count: u32) -> bool {
    count == 0 || most(payer, counter) >= count
}

/// What a snapshot of the source lists once the counters are removed, so the ability remembers its source as it stands
/// after the cost is paid (CR 113.7a).
pub(crate) fn after_removal(
    counters: &mut Vec<(CounterKind, u32)>,
    counter: &CounterKind,
    count: u32,
) {
    for (kind, held) in counters.iter_mut() {
        if kind == counter {
            *held = held.saturating_sub(count);
        }
    }
    counters.retain(|(_, held)| *held > 0);
}

/// The counters removed; removing none is no event.
pub(super) fn events(
    state: &GameState,
    source: ObjectId,
    counter: &CounterKind,
    count: u32,
) -> Vec<Event> {
    let (Some(object), Ok(removed)) = (state.object(source), i32::try_from(count)) else {
        return Vec::new();
    };
    if removed == 0 {
        return Vec::new();
    }
    vec![Event::CountersChanged(CountersChanged {
        on: Target::Object(object.reference()),
        counter: counter.clone(),
        delta: -removed,
    })]
}
