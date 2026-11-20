//! Counters put or removed "this way": a resolution (or the effects a replacement runs) records the counters its steps
//! put on and removed from each object, and a delayed or reflexive trigger it sets up carries the record as bound names
//! (CR 603.7c). `Effect::R110RemovePutThisWay` removes again, from each object, the counters put on it this way (Bounty
//! of the Hunt, Shambling Swarm); `Amount::R110RemovedThisWay` counts the counters removed this way (Magma Pummeler).

use mtg_cards::effect::Effect;
use mtg_cards::value::CounterKind;

use super::wrong_effect;
use crate::choose::Target;
use crate::counters::CountersChanged;
use crate::events::Event;
use crate::object::ObjectRef;
use crate::resolve::{Bindings, Cx, ResolveError, Resolving, Run};

/// Counters of a kind on one object, as a run records them.
pub(crate) type R110Counted = Vec<(ObjectRef, CounterKind, u32)>;

/// The prefix of a bound name that carries counters a resolution put into the trigger it sets up:
/// `?r110put:<index>:<count>:<kind as JSON>`, bound to the object they were put on.
pub(crate) const R110_PUT: &str = "?r110put:";

/// The prefix of a bound name that carries counters a resolution removed, as `R110_PUT` does.
pub(crate) const R110_REMOVED: &str = "?r110removed:";

/// The bound names that carry what a run put and removed into a trigger it sets up, one for each object and kind.
pub(crate) fn r110_carried(resolving: &Resolving) -> Vec<(String, Vec<Target>)> {
    let named = |prefix: &str, counted: &R110Counted| -> Vec<(String, Vec<Target>)> {
        counted
            .iter()
            .enumerate()
            .filter_map(|(index, (object, kind, count))| {
                let kind = serde_json::to_string(kind).ok()?;
                Some((
                    format!("{prefix}{index}:{count}:{kind}"),
                    vec![Target::Object(*object)],
                ))
            })
            .collect()
    };
    let mut carried = named(R110_PUT, &resolving.r110_put);
    carried.extend(named(R110_REMOVED, &resolving.r110_removed));
    carried
}

/// Adds a change of counters on an object to what a run has put or removed so far; a player's counters are not kept.
pub(crate) fn r110_note(resolving: &mut Resolving, changed: &CountersChanged) {
    let Target::Object(object) = changed.on else {
        return;
    };
    let counted = if changed.delta > 0 {
        &mut resolving.r110_put
    } else {
        &mut resolving.r110_removed
    };
    let count = changed.delta.unsigned_abs();
    match counted
        .iter_mut()
        .find(|(named, held, _)| *named == object && *held == changed.counter)
    {
        Some((_, _, total)) => *total = total.saturating_add(count),
        None => counted.push((object, changed.counter.clone(), count)),
    }
}

/// The objects and counts of `counter` this run recorded in `own`, and those the run that set it up carried under
/// `prefix` in `bindings`.
pub(crate) fn r110_this_way(
    own: &R110Counted,
    bindings: &Bindings,
    prefix: &str,
    counter: &CounterKind,
) -> Result<Vec<(ObjectRef, u32)>, ResolveError> {
    let mut found: Vec<(ObjectRef, u32)> = own
        .iter()
        .filter(|(_, kind, _)| kind == counter)
        .map(|(object, _, count)| (*object, *count))
        .collect();
    for (name, targets) in bindings {
        let Some(rest) = name.strip_prefix(prefix) else {
            continue;
        };
        let mut parts = rest.splitn(3, ':');
        let (_, count, kind) = (parts.next(), parts.next(), parts.next());
        let count = count.and_then(|count| count.parse::<u32>().ok());
        let kind = kind.and_then(|kind| serde_json::from_str::<CounterKind>(kind).ok());
        let (Some(count), Some(kind)) = (count, kind) else {
            return Err(ResolveError::Invalid(format!("`{name}` names no counters")));
        };
        if kind != *counter {
            continue;
        }
        found.extend(targets.iter().filter_map(|target| match target {
            Target::Object(object) => Some((*object, count)),
            Target::Player(_) => None,
        }));
    }
    Ok(found)
}

/// Removes, from each object that is still the very object it was, as many counters of the kind as were put on it this
/// way, no more than it has.
pub(super) fn run(cx: &Cx<'_>, effect: &Effect) -> Result<Run, ResolveError> {
    let Effect::R110RemovePutThisWay { counter } = effect else {
        return Err(wrong_effect("R110RemovePutThisWay"));
    };
    let put = r110_this_way(
        &cx.resolving.r110_put,
        cx.resolving.bindings(),
        R110_PUT,
        counter,
    )?;
    let events = put
        .into_iter()
        .filter_map(|(object, count)| {
            let held = crate::counters::held_by(cx.state(), Target::Object(object), counter);
            let removed = i32::try_from(count.min(held)).ok().filter(|n| *n > 0)?;
            Some(Event::CountersChanged(CountersChanged {
                on: Target::Object(object),
                counter: counter.clone(),
                delta: -removed,
            }))
        })
        .collect();
    Ok(Run::events(events))
}
