//! Counter moves by kind and by "one or more" (CR 122.5): `Effect::R110MoveEachKindMissing` moves one counter of each
//! kind the destination lacks, `Effect::R110MoveOneOrMore` has the controller move counters one at a time, at least one.

use mtg_cards::effect::Effect;
use mtg_cards::refs::Ref;

use super::put_counters::holders;
use super::wrong_effect;
use crate::choose::Target;
use crate::counters::{CountersChanged, kinds_held};
use crate::events::Event;
use crate::resolve::{Cx, ResolveError, Run};

/// The one object `from` names that gives counters up and the one `to` names that takes them, both permanents and not
/// the same; none when either is missing.
fn ends(cx: &Cx<'_>, from: &Ref, to: &Ref) -> Result<Option<(Target, Target)>, ResolveError> {
    let first = |named: &Ref| -> Result<Option<Target>, ResolveError> {
        Ok(holders(cx, named)?
            .into_iter()
            .find(|target| target.object().is_some()))
    };
    Ok(match (first(from)?, first(to)?) {
        (Some(source), Some(target)) if source != target => Some((source, target)),
        _ => None,
    })
}

/// Each kind `from` holds that `to` holds none of moves one counter, all in one step and in the fixed order of the kinds.
pub(super) fn run_each_kind(cx: &Cx<'_>, effect: &Effect) -> Result<Run, ResolveError> {
    let Effect::R110MoveEachKindMissing { from, to } = effect else {
        return Err(wrong_effect("R110MoveEachKindMissing"));
    };
    let Some((source, target)) = ends(cx, from, to)? else {
        return Ok(Run::events(Vec::new()));
    };
    let has: Vec<_> = kinds_held(cx.state(), target)
        .into_iter()
        .filter(|(_, n)| *n > 0)
        .map(|(kind, _)| kind)
        .collect();
    let mut events = Vec::new();
    for (kind, held) in kinds_held(cx.state(), source) {
        if held == 0 || has.contains(&kind) {
            continue;
        }
        for (on, delta) in [(source, -1), (target, 1)] {
            events.push(Event::CountersChanged(CountersChanged {
                on,
                counter: kind.clone(),
                delta,
            }));
        }
    }
    Ok(Run::events(events))
}

/// The controller picks the counters of `from` one at a time and may stop once one is picked; each goes onto `to`.
pub(super) fn run_one_or_more(cx: &Cx<'_>, effect: &Effect) -> Result<Run, ResolveError> {
    let Effect::R110MoveOneOrMore { from, to } = effect else {
        return Err(wrong_effect("R110MoveOneOrMore"));
    };
    let Some((source, target)) = ends(cx, from, to)? else {
        return Ok(Run::events(Vec::new()));
    };
    let all: u32 = kinds_held(cx.state(), source).iter().map(|(_, n)| *n).sum();
    let taken =
        match super::w2f_any_counters::w2f_pick_least(cx, vec![source], None, (all, true), true)? {
            Run::Done { events, .. } => events,
            asked => return Ok(asked),
        };
    let mut events = Vec::new();
    for event in taken {
        let Event::CountersChanged(changed) = event else {
            continue;
        };
        let moved = changed.delta.saturating_neg();
        if moved > 0 {
            let onto = CountersChanged {
                on: target,
                counter: changed.counter.clone(),
                delta: moved,
            };
            events.push(Event::CountersChanged(changed));
            events.push(Event::CountersChanged(onto));
        }
    }
    Ok(Run::events(events))
}
