//! Counters on several targets in one sentence, a number for each ("Put a -1/-1 counter on target creature, two -1/-1
//! counters on another target creature, and three -1/-1 counters on a third target creature."), and a fight between two
//! targets.

use super::{Effect, Ref, TargetSpec, counters, join_list, multi, spec_text};

/// What the ordinals before "target" say for the targets after the first.
const AFTER_THE_FIRST: [&str; 2] = ["another target ", "a third target "];

/// The sentence for counters put on each of two or three targets, one effect for each target in order.
pub(super) fn text(targets: &[TargetSpec], effects: &[Effect]) -> Option<String> {
    if !(2..=3).contains(&targets.len()) || effects.len() != targets.len() {
        return None;
    }
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
            Some(later) => {
                let ordinal = AFTER_THE_FIRST.get(later)?;
                let named = place.strip_prefix(AFTER_THE_FIRST[0])?;
                format!("{ordinal}{named}")
            }
        };
        parts.push(format!("{} on {place}", counters::text(counter, count)?));
    }
    Some(format!("put {}", join_list(&parts)))
}

/// The ways the card words the fight after the sentences about its first target.
const FIGHTS: [&str; 4] = [
    "then it fights",
    "it fights",
    "then that creature fights",
    "that creature fights",
];

/// Whether the effect refers to a target.
fn uses_a_target(effect: &Effect) -> bool {
    let mut found = Vec::new();
    if let Ok(value) = serde_json::to_value(effect) {
        multi::targets_used(&value, &mut found);
    }
    !found.is_empty()
}

/// The ways to write effects that name no target, as the sentences of a clause without targets.
fn targetless(effects: &[Effect]) -> Option<Vec<Vec<String>>> {
    if effects.is_empty() {
        return Some(vec![Vec::new()]);
    }
    let written = super::alternatives(&[], effects);
    (!written.is_empty()).then_some(written)
}

/// Every way to follow one of `firsts` with one of `seconds`.
fn then(firsts: &[Vec<String>], seconds: &[Vec<String>]) -> Vec<Vec<String>> {
    firsts
        .iter()
        .flat_map(|first| {
            seconds
                .iter()
                .map(move |second| [first.clone(), second.clone()].concat())
        })
        .collect()
}

/// A fight of the two targets with sentences around it: "Proliferate. Target creature you control fights up to one target
/// creature you don't control.", "Put a +1/+1 counter on target creature you control. Then that creature fights target
/// creature you don't control.", "Target creature you control fights target creature you don't control. Create a
/// Treasure token." The sentences before that name the first target are written for it alone; the fight then says "it".
pub(super) fn fight_among(targets: &[TargetSpec], effects: &[Effect]) -> Option<Vec<Vec<String>>> {
    let [first, second] = targets else {
        return None;
    };
    let at = effects.iter().position(|effect| {
        matches!(
            effect,
            Effect::Fight {
                what: Ref::Target(0),
                with: Ref::Target(1)
            }
        )
    })?;
    let (before, after) = (&effects[..at], &effects[at + 1..]);
    let free = before
        .iter()
        .take_while(|effect| !uses_a_target(effect))
        .count();
    (!after.iter().any(uses_a_target)).then_some(())?;
    let (free, about_the_first) = before.split_at(free);
    let opponent = spec_text(second)?;
    let fights: Vec<Vec<String>> = if about_the_first.is_empty() {
        vec![vec![format!("{} fights {opponent}", spec_text(first)?)]]
    } else {
        let mut all = Vec::new();
        for sentences in super::alternatives(std::slice::from_ref(first), about_the_first) {
            for fights in FIGHTS {
                all.push([sentences.clone(), vec![format!("{fights} {opponent}")]].concat());
            }
        }
        all
    };
    let found = then(&then(&targetless(free)?, &fights), &targetless(after)?);
    (!found.is_empty()).then_some(found)
}
