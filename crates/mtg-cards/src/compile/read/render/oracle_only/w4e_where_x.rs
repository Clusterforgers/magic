//! A count of counters as X, named at the end of the sentence that says X: "You draw X cards and you lose X life, where
//! X is the number of counters on that creature.", "create X tapped 2/1 ... tokens with flying, where X is the number of
//! counters it had on it", "Create X 2/2 ... tokens, where X is the total number of rad counters among players." and
//! "Put X +1/+1 counters on it, where X is the number of growth counters on ~."

use serde_json::Value;

use super::super::super::counters::kind_text;
use super::super::super::selectors::spec_text;
use crate::ability::Ability;
use crate::refs::{Ref, TargetSpec};
use crate::value::Amount;

/// The wordings of an ability with one count of counters as its amount; `bodies` writes the ability with X in its place.
pub(super) fn wordings(ability: &Ability, bodies: impl Fn(&Ability) -> Vec<String>) -> Vec<String> {
    let Ok(json) = serde_json::to_value(ability) else {
        return Vec::new();
    };
    let mut found = Vec::new();
    counter_counts(&json, &mut found);
    let [count] = found.as_slice() else {
        return Vec::new();
    };
    let Ok(amount) = serde_json::from_value::<Amount>(count.clone()) else {
        return Vec::new();
    };
    let Some(said) = w4e_named(&amount, targets(ability)) else {
        return Vec::new();
    };
    let mut replaced = json.clone();
    let Ok(x) = serde_json::to_value(Amount::X) else {
        return Vec::new();
    };
    if !replace(&mut replaced, count, &x) {
        return Vec::new();
    }
    let Ok(replaced) = serde_json::from_value::<Ability>(replaced) else {
        return Vec::new();
    };
    let tail = format!(", where X is {said}.");
    bodies(&replaced)
        .iter()
        .filter_map(|text| {
            let head = text.strip_suffix('.')?;
            let last = head.rsplit(['.', '\n']).next().unwrap_or(head);
            last.contains(" X ").then(|| format!("{head}{tail}"))
        })
        .collect()
}

/// The targets of the ability.
fn targets(ability: &Ability) -> &[TargetSpec] {
    match ability {
        Ability::Spell { targets, .. }
        | Ability::Activated { targets, .. }
        | Ability::Triggered { targets, .. } => targets,
        _ => &[],
    }
}

/// What X is: "the number of counters on that creature", "the number of counters it had on it", "the total number of
/// rad counters among players", "the number of growth counters on ~".
fn w4e_named(amount: &Amount, targets: &[TargetSpec]) -> Option<String> {
    match amount {
        Amount::AnyCounters(Ref::Triggering) => {
            Some("the number of counters it had on it".to_owned())
        }
        Amount::AnyCounters(Ref::Target(index)) => {
            let target = spec_text(targets.get(usize::from(*index))?)?;
            let noun = target.strip_prefix("target ")?;
            Some(format!("the number of counters on that {noun}"))
        }
        Amount::Counters {
            on: Ref::EachPlayer,
            counter,
        } => Some(format!(
            "the total number of {} counters among players",
            kind_text(counter)?
        )),
        Amount::Counters {
            on: Ref::This,
            counter,
        } => Some(format!(
            "the number of {} counters on ~",
            kind_text(counter)?
        )),
        _ => None,
    }
}

/// The counts of counters in the data, each once.
fn counter_counts(value: &Value, found: &mut Vec<Value>) {
    match value {
        Value::Object(map)
            if map.len() == 1
                && (map.contains_key("AnyCounters") || map.contains_key("Counters")) =>
        {
            if !found.contains(value) {
                found.push(value.clone());
            }
        }
        Value::Object(map) => map.values().for_each(|child| counter_counts(child, found)),
        Value::Array(items) => items.iter().for_each(|child| counter_counts(child, found)),
        _ => {}
    }
}

/// Puts `into` in the place of every `from` in the data; whether there was one.
fn replace(value: &mut Value, from: &Value, into: &Value) -> bool {
    if value == from {
        value.clone_from(into);
        return true;
    }
    let children: Box<dyn Iterator<Item = &mut Value>> = match value {
        Value::Array(items) => Box::new(items.iter_mut()),
        Value::Object(map) => Box::new(map.values_mut()),
        _ => return false,
    };
    children.fold(false, |found, child| replace(child, from, into) | found)
}
