//! `Effect::R110RemovePutThisWay`: for each counter of a kind the steps before it put on an object, one is removed
//! from that object again (Bounty of the Hunt, Shambling Swarm; CR 603.7c).

use mtg_cards::effect::Effect;
use mtg_cards::value::CounterKind;

use super::wrong_effect;
use crate::choose::Target;
use crate::counters::CountersChanged;
use crate::events::Event;
use crate::object::ObjectRef;
use crate::resolve::{Cx, ResolveError, Run};

/// The prefix of a bound name that carries counters a resolution put into the delayed trigger it sets up:
/// `?r110put:<index>:<count>:<kind as JSON>`, bound to the object they were put on.
pub(crate) const R110_PUT: &str = "?r110put:";

/// The bound names that carry the counters a resolution put into a delayed trigger, one for each object and kind.
pub(crate) fn r110_carried(put: &[(ObjectRef, CounterKind, u32)]) -> Vec<(String, Vec<Target>)> {
    put.iter()
        .enumerate()
        .filter_map(|(index, (object, kind, count))| {
            let kind = serde_json::to_string(kind).ok()?;
            Some((
                format!("{R110_PUT}{index}:{count}:{kind}"),
                vec![Target::Object(*object)],
            ))
        })
        .collect()
}

/// Adds `count` counters of `kind` put on `object` to what a resolution has put so far.
pub(crate) fn r110_note_put(
    put: &mut Vec<(ObjectRef, CounterKind, u32)>,
    object: ObjectRef,
    kind: &CounterKind,
    count: u32,
) {
    match put
        .iter_mut()
        .find(|(named, held, _)| *named == object && held == kind)
    {
        Some((_, _, total)) => *total = total.saturating_add(count),
        None => put.push((object, kind.clone(), count)),
    }
}

/// The objects and counts of `counter` put this way: by this resolution, and by the resolution that set this delayed
/// trigger up (carried as bound names).
fn put_this_way(cx: &Cx<'_>, counter: &CounterKind) -> Result<Vec<(ObjectRef, u32)>, ResolveError> {
    let mut put: Vec<(ObjectRef, u32)> = cx
        .resolving
        .r110_put
        .iter()
        .filter(|(_, kind, _)| kind == counter)
        .map(|(object, _, count)| (*object, *count))
        .collect();
    for (name, targets) in cx.resolving.bindings() {
        let Some(rest) = name.strip_prefix(R110_PUT) else {
            continue;
        };
        let mut parts = rest.splitn(3, ':');
        let (_, count, kind) = (parts.next(), parts.next(), parts.next());
        let count = count.and_then(|count| count.parse::<u32>().ok());
        let kind = kind.and_then(|kind| serde_json::from_str::<CounterKind>(kind).ok());
        let (Some(count), Some(kind)) = (count, kind) else {
            return Err(ResolveError::Invalid(format!(
                "`{name}` names no counters put"
            )));
        };
        if kind != *counter {
            continue;
        }
        put.extend(targets.iter().filter_map(|target| match target {
            Target::Object(object) => Some((*object, count)),
            Target::Player(_) => None,
        }));
    }
    Ok(put)
}

/// Removes, from each object that is still the very object it was, as many counters of the kind as were put on it this
/// way, no more than it has.
pub(super) fn run(cx: &Cx<'_>, effect: &Effect) -> Result<Run, ResolveError> {
    let Effect::R110RemovePutThisWay { counter } = effect else {
        return Err(wrong_effect("R110RemovePutThisWay"));
    };
    let events = put_this_way(cx, counter)?
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
