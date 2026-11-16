//! Counter caps on permanents ("~ can't have more than seven dream counters on it."): after the replacement effects, a
//! put of counters that would pass a cap in force puts only enough to reach it, and none once it is reached.

use crate::choose::Target;
use crate::counters::CountersChanged;
use crate::events::Event;
use crate::object::GameObject;
use crate::state::GameState;
use mtg_cards::value::CounterKind;
use mtg_core::ids::ObjectId;

/// The events with each put of counters on a capped permanent cut to what its cap leaves room for, counting the changes
/// of the batch before it; a put cut to nothing is dropped.
pub(crate) fn r6500_capped(state: &GameState, events: Vec<Event>) -> Vec<Event> {
    if !crate::rules::r6500_any_counter_caps(state) {
        return events;
    }
    let mut held: Vec<(ObjectId, CounterKind, i64)> = Vec::new();
    let mut out = Vec::with_capacity(events.len());
    for event in events {
        let Event::CountersChanged(CountersChanged {
            on: Target::Object(on),
            counter,
            delta,
        }) = &event
        else {
            out.push(event);
            continue;
        };
        let at = if let Some(at) = held
            .iter()
            .position(|(id, kind, _)| *id == on.id && kind == counter)
        {
            at
        } else {
            let now = state
                .object(on.id)
                .and_then(GameObject::permanent)
                .map_or(0, |permanent| permanent.counters_of(counter));
            held.push((on.id, counter.clone(), i64::from(now)));
            held.len() - 1
        };
        let cap = crate::rules::r6500_counter_cap(state, on.id, counter);
        let mut change = i64::from(*delta);
        if let Some(most) = cap.filter(|_| change > 0) {
            change = change.min((i64::from(most) - held[at].2).max(0));
        }
        held[at].2 += change;
        if change == 0 {
            continue;
        }
        let Ok(delta) = i32::try_from(change) else {
            continue;
        };
        out.push(Event::CountersChanged(CountersChanged {
            on: Target::Object(*on),
            counter: counter.clone(),
            delta,
        }));
    }
    out
}
