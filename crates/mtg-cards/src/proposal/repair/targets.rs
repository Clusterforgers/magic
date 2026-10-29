//! Target indexes: references that count from the wrong start are renumbered when exactly one renumbering makes them
//! match the declared targets, and `Triggering` in an ability that is not triggered stands for the one target no effect
//! uses.

use std::collections::BTreeSet;

use serde_json::{Map, Value, json};

use super::{Repair, RepairClass, body_of};

/// The keys of a scope of its own: modes, tokens and emblems with abilities hold targets of their own.
const SCOPES: [&str; 4] = ["targets", "Modal", "modes", "abilities"];

/// The index of an object that is exactly `{"Target": n}`.
fn target_index(map: &Map<String, Value>) -> Option<u64> {
    if map.len() == 1 {
        map.get("Target")?.as_u64()
    } else {
        None
    }
}

/// Whether the value holds a scope of its own at any depth.
fn scoped(value: &Value) -> bool {
    match value {
        Value::Object(map) => map
            .iter()
            .any(|(key, inner)| SCOPES.contains(&key.as_str()) || scoped(inner)),
        Value::Array(items) => items.iter().any(scoped),
        _ => false,
    }
}

/// The indexes the references of the value use.
fn used(value: &Value, found: &mut BTreeSet<u64>) {
    match value {
        Value::Object(map) => match target_index(map) {
            Some(index) => {
                found.insert(index);
            }
            None => map.values().for_each(|inner| used(inner, found)),
        },
        Value::Array(items) => items.iter().for_each(|item| used(item, found)),
        _ => {}
    }
}

/// Moves every reference down by `by`.
fn shift(value: &mut Value, by: u64) {
    match value {
        Value::Object(map) => match target_index(map) {
            Some(index) => {
                map.insert("Target".to_owned(), index.saturating_sub(by).into());
            }
            None => map.values_mut().for_each(|inner| shift(inner, by)),
        },
        Value::Array(items) => items.iter_mut().for_each(|item| shift(item, by)),
        _ => {}
    }
}

/// Replaces `Triggering` by a reference to the target `index`, and says how many it replaced.
fn trigger_to_target(value: &mut Value, index: u64) -> usize {
    match value {
        Value::String(word) if word == "Triggering" => {
            *value = json!({ "Target": index });
            1
        }
        Value::Object(map) => map
            .values_mut()
            .map(|inner| trigger_to_target(inner, index))
            .sum(),
        Value::Array(items) => items
            .iter_mut()
            .map(|item| trigger_to_target(item, index))
            .sum(),
        _ => 0,
    }
}

/// The indexes the effects of the body use: a target is chosen for the effects, so a reference elsewhere (the event of a
/// trigger) is no use of it.
fn used_by(body: &Map<String, Value>) -> BTreeSet<u64> {
    let mut found = BTreeSet::new();
    if let Some(effects) = body.get("effects") {
        used(effects, &mut found);
    }
    found
}

/// Repairs the target references of one ability.
pub(super) fn repair(ability: &mut Value, at: &str, repairs: &mut Vec<Repair>) {
    let Some((name, body)) = body_of(ability) else {
        return;
    };
    let triggered = name == "Triggered";
    if !["Spell", "Triggered", "Activated", "Loyalty"].contains(&name) || body.values().any(scoped)
    {
        return;
    }
    let count = body
        .get("targets")
        .and_then(Value::as_array)
        .map_or(0, |targets| {
            u64::try_from(targets.len()).unwrap_or(u64::MAX)
        });
    let uses = used_by(body);
    if let (Some(&first), Some(&last)) = (uses.first(), uses.last())
        && first > 0
        && count > 0
        && u64::try_from(uses.len()).is_ok_and(|n| n == count)
        && last - first + 1 == count
    {
        if let Some(effects) = body.get_mut("effects") {
            shift(effects, first);
        }
        repairs.push(Repair {
            class: RepairClass::TargetIndex,
            at: at.to_owned(),
            note: format!(
                "the targets are used as {first}..={last} and declared as 0..{count}: renumbered"
            ),
        });
    }
    if triggered {
        return;
    }
    let uses = used_by(body);
    let unused: Vec<u64> = (0..count).filter(|index| !uses.contains(index)).collect();
    if let [index] = unused[..] {
        let replaced = body
            .get_mut("effects")
            .map_or(0, |effects| trigger_to_target(effects, index));
        if replaced > 0 {
            repairs.push(Repair {
                class: RepairClass::TriggeringTarget,
                at: at.to_owned(),
                note: format!("`Triggering` outside a triggered ability is the target {index}, which no effect uses"),
            });
        }
    }
}
