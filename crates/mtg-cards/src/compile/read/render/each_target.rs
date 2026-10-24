//! Counters on several targets in one sentence, a number for each ("Put a -1/-1 counter on target creature, two -1/-1
//! counters on another target creature, and three -1/-1 counters on a third target creature."; "Put three +1/+1 counters
//! on target creature you control and three +1/+1 counters on target creature an opponent controls."), and a fight
//! between two targets.

use super::{Effect, Ref, TargetSpec, counters, join_list, spec_text};

/// What the ordinals before "target" say for the targets after the first.
const AFTER_THE_FIRST: [&str; 2] = ["another target ", "a third target "];

/// "Target creature you control fights another target creature.": one creature fights the other of two targets.
fn fight(targets: &[TargetSpec], effects: &[Effect]) -> Option<String> {
    let ([first, second], [Effect::Fight { what, with }]) = (targets, effects) else {
        return None;
    };
    (*what == Ref::Target(0) && *with == Ref::Target(1)).then_some(())?;
    Some(format!(
        "{} fights {}",
        spec_text(first)?,
        spec_text(second)?
    ))
}

/// The sentence for counters put on each of two or three targets, one effect for each target in order, or for a fight
/// between two targets.
pub(super) fn text(targets: &[TargetSpec], effects: &[Effect]) -> Option<String> {
    fight(targets, effects).or_else(|| counters_on_each(targets, effects, false))
}

/// The same sentence for targets that tell themselves apart by who controls them, so that none is "another": "put three
/// +1/+1 counters on target creature you control and three +1/+1 counters on target creature an opponent controls".
pub(super) fn on_different(targets: &[TargetSpec], effects: &[Effect]) -> Option<String> {
    counters_on_each(targets, effects, true)
}

/// Counters on each target in order. A target after the first is "another target ..." (`apart` false) or, when it differs
/// from every earlier one in its words, "target ..." (`apart` true).
fn counters_on_each(targets: &[TargetSpec], effects: &[Effect], apart: bool) -> Option<String> {
    if !(2..=3).contains(&targets.len()) || effects.len() != targets.len() {
        return None;
    }
    let mut places: Vec<String> = Vec::with_capacity(targets.len());
    let mut parts = Vec::with_capacity(targets.len());
    for (at, (spec, effect)) in targets.iter().zip(effects).enumerate() {
        let Effect::PutCounters {
            on: Ref::Target(number),
            counter,
            count,
        } = effect
        else {
            return None;
        };
        (usize::from(*number) == at).then_some(())?;
        let place = spec_text(spec)?;
        let place = match at.checked_sub(1) {
            None => place,
            Some(_) if apart => {
                (place.starts_with("target ") && !places.contains(&place)).then_some(())?;
                place
            }
            Some(later) => {
                let ordinal = AFTER_THE_FIRST.get(later)?;
                let named = place.strip_prefix(AFTER_THE_FIRST[0])?;
                format!("{ordinal}{named}")
            }
        };
        parts.push(format!("{} on {place}", counters::text(counter, count)?));
        places.push(place);
    }
    Some(format!("put {}", join_list(&parts)))
}
