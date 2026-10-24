//! Several targets: the effects of each target are written as sentences of their own, each naming its target in full
//! ("Exile target artifact or enchantment. Put X +1/+1 counters on target creature you control, ..."). The effects of
//! the target `n` are written as if it were the only one.

use serde_json::Value;

use super::{Effect, TargetSpec};

/// The ways to write the effects of a clause with several targets as sentences, which are the ways to write the
/// effects of each target joined, and counters on targets that differ in one sentence; empty when an effect acts on
/// more than one target or the targets are not used in order.
pub(super) fn alternatives(targets: &[TargetSpec], effects: &[Effect]) -> Vec<Vec<String>> {
    if let Some(sentence) = super::each_target::text(targets, effects)
        .or_else(|| super::target_counts::text(targets, effects))
        .or_else(|| super::shuffled_in::text(targets, effects))
    {
        return vec![vec![sentence]];
    }
    if let Some(options) = super::each_target::fight_among(targets, effects) {
        return options;
    }
    let mut all = by_target(targets, effects);
    all.extend(super::each_target::on_different(targets, effects).map(|sentence| vec![sentence]));
    all
}

/// The sentences of each target's effects, one after the other.
fn by_target(targets: &[TargetSpec], effects: &[Effect]) -> Vec<Vec<String>> {
    let Some(parts) = split(targets.len(), effects) else {
        return Vec::new();
    };
    let mut all: Vec<Vec<String>> = vec![Vec::new()];
    for (spec, part) in targets.iter().zip(&parts) {
        let options = super::alternatives(std::slice::from_ref(spec), part);
        if options.is_empty() {
            return Vec::new();
        }
        all = all
            .into_iter()
            .flat_map(|so_far| {
                options
                    .iter()
                    .map(move |option| [so_far.clone(), option.clone()].concat())
            })
            .collect();
    }
    all
}

/// The effects of each target, with that target renumbered as the first. An effect that names no target joins the
/// target before it; every target has an effect and no effect names two.
fn split(count: usize, effects: &[Effect]) -> Option<Vec<Vec<Effect>>> {
    let mut parts: Vec<Vec<Effect>> = vec![Vec::new()];
    for effect in effects {
        let mut value = serde_json::to_value(effect).ok()?;
        let mut used = Vec::new();
        targets_used(&value, &mut used);
        used.sort_unstable();
        used.dedup();
        let current = parts.len() - 1;
        match used.as_slice() {
            [] => {}
            [next] if usize::from(*next) == current => {}
            [next] if usize::from(*next) == current + 1 => parts.push(Vec::new()),
            _ => return None,
        }
        renumber(&mut value, u64::try_from(parts.len() - 1).ok()?);
        parts.last_mut()?.push(serde_json::from_value(value).ok()?);
    }
    (parts.len() == count && parts.iter().all(|part| !part.is_empty())).then_some(parts)
}

/// The numbers of the targets the data refers to.
pub(super) fn targets_used(value: &Value, found: &mut Vec<u8>) {
    match value {
        Value::Object(map) => {
            if let (1, Some(Value::Number(number))) = (map.len(), map.get("Target"))
                && let Some(number) = number.as_u64().and_then(|n| u8::try_from(n).ok())
            {
                found.push(number);
            }
            map.values().for_each(|child| targets_used(child, found));
        }
        Value::Array(items) => items.iter().for_each(|child| targets_used(child, found)),
        _ => {}
    }
}

/// The reference to target `from` written as a reference to the first target.
fn renumber(value: &mut Value, from: u64) {
    match value {
        Value::Object(map) => {
            if map.len() == 1
                && let Some(number) = map.get_mut("Target")
                && number.as_u64() == Some(from)
            {
                *number = Value::from(0);
            }
            map.values_mut().for_each(|child| renumber(child, from));
        }
        Value::Array(items) => items.iter_mut().for_each(|child| renumber(child, from)),
        _ => {}
    }
}
