//! The counters an ability's snapshot holds once a replacement has changed what its cost put on the source: Vorinclex
//! makes a `+1` of a planeswalker put two, so the ability remembers its source with two more, not one more (CR 113.7a).

use std::collections::{BTreeMap, BTreeSet};

use mtg_cards::value::CounterKind;
use mtg_core::ids::ObjectId;

use crate::choose::Target;
use crate::events::Event;

/// The change of `kind` counters on `source` that the events of a batch make.
fn moved(batch: &[Event], source: ObjectId, kind: &CounterKind) -> i64 {
    batch
        .iter()
        .filter_map(|event| match event {
            Event::CountersChanged(changed)
                if changed.counter == *kind
                    && matches!(changed.on, Target::Object(on) if on.id == source) =>
            {
                Some(i64::from(changed.delta))
            }
            _ => None,
        })
        .sum()
}

/// Moves the counters the snapshot of each ability put on the stack by `events` holds by what the replacements made of
/// the counters `original` put on its source: more or fewer, or none when they were all replaced away.
pub(super) fn resettle(original: &[Event], events: &mut [Event]) {
    let fixes: Vec<(usize, Vec<(CounterKind, i64)>)> = events
        .iter()
        .enumerate()
        .filter_map(|(at, event)| {
            let Event::AbilityPutOnStack(put) = event else {
                return None;
            };
            let source = put.source.id;
            let kinds: BTreeSet<CounterKind> = original
                .iter()
                .filter_map(|event| match event {
                    Event::CountersChanged(changed)
                        if matches!(changed.on, Target::Object(on) if on.id == source) =>
                    {
                        Some(changed.counter.clone())
                    }
                    _ => None,
                })
                .collect();
            let by: Vec<(CounterKind, i64)> = kinds
                .into_iter()
                .filter_map(|kind| {
                    let by = moved(events, source, &kind) - moved(original, source, &kind);
                    (by != 0).then_some((kind, by))
                })
                .collect();
            (!by.is_empty()).then_some((at, by))
        })
        .collect();
    for (at, by) in fixes {
        let Event::AbilityPutOnStack(put) = &mut events[at] else {
            continue;
        };
        let mut held: BTreeMap<CounterKind, u32> = put.snapshot.counters.drain(..).collect();
        for (kind, by) in by {
            let now = (i64::from(held.get(&kind).copied().unwrap_or(0)) + by)
                .clamp(0, i64::from(u32::MAX));
            match u32::try_from(now) {
                Ok(now) if now > 0 => {
                    held.insert(kind, now);
                }
                _ => {
                    held.remove(&kind);
                }
            }
        }
        put.snapshot.counters.extend(held);
    }
}
