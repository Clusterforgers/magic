//! A declared target no effect uses is dropped when the printed words of the ability have no "target", and is wired
//! to the one effect that names the ability's own object where the text has the phrase.

use std::collections::BTreeSet;

use serde_json::{Map, Value, json};

use super::targets::{scoped, used_by};
use super::text::{ability_text, has_word};
use super::{EFFECT_ABILITIES, Repair, RepairClass, Site, body_of};

/// The fields of an effect that hold the object it acts on.
const OBJECT_SLOTS: [&str; 4] = ["what", "to", "on", "affected"];

/// The references that stand for the ability's own object, where the text names a target.
const OWN: [&str; 2] = ["This", "Triggering"];

/// Whether a reference to a target stands outside the effects and the declarations.
fn used_elsewhere(body: &Map<String, Value>) -> bool {
    body.iter()
        .filter(|(key, _)| !["effects", "targets"].contains(&key.as_str()))
        .any(|(_, value)| {
            let mut found = BTreeSet::new();
            super::targets::used(value, &mut found);
            !found.is_empty()
        })
}

/// Moves every reference down by the number of dropped targets before it.
fn compact(value: &mut Value, dropped: &[usize]) {
    match value {
        Value::Object(map) => {
            let index = (map.len() == 1)
                .then(|| map.get("Target").and_then(Value::as_u64))
                .flatten();
            match index {
                Some(index) => {
                    let below = dropped
                        .iter()
                        .filter(|d| u64::try_from(**d).is_ok_and(|d| d < index))
                        .count();
                    let moved = index.saturating_sub(u64::try_from(below).unwrap_or(0));
                    map.insert("Target".to_owned(), moved.into());
                }
                None => map.values_mut().for_each(|inner| compact(inner, dropped)),
            }
        }
        Value::Array(items) => items.iter_mut().for_each(|item| compact(item, dropped)),
        _ => {}
    }
}

/// The object slots of the top-level effects that hold the ability's own object.
fn own_slots(effects: &mut [Value]) -> Vec<&mut Value> {
    effects
        .iter_mut()
        .filter_map(|effect| body_of(effect).map(|(_, fields)| fields))
        .flat_map(|fields| fields.iter_mut())
        .filter(|(key, value)| {
            OBJECT_SLOTS.contains(&key.as_str())
                && value.as_str().is_some_and(|word| OWN.contains(&word))
        })
        .map(|(_, value)| value)
        .collect()
}

/// Whether the declaration is a target of objects.
fn of_objects(spec: &Value) -> bool {
    spec.get("what")
        .and_then(Value::as_object)
        .is_some_and(|what| what.contains_key("Object"))
}

/// Repairs the targets of one ability that no effect uses.
pub(super) fn repair(ability: &mut Value, site: &Site<'_>, repairs: &mut Vec<Repair>) {
    let Some((name, body)) = body_of(ability) else {
        return;
    };
    if !EFFECT_ABILITIES.contains(&name) || body.values().any(scoped) || used_elsewhere(body) {
        return;
    }
    let count = body
        .get("targets")
        .and_then(Value::as_array)
        .map_or(0, Vec::len);
    let uses = used_by(body);
    let unused: Vec<usize> = (0..count)
        .filter(|index| u64::try_from(*index).is_ok_and(|i| !uses.contains(&i)))
        .collect();
    if unused.is_empty() {
        return;
    }
    let Some(text) = ability_text(site.face, name, site.nth, site.count) else {
        return;
    };
    if !has_word(&text, "target") && !has_word(&text, "targets") {
        if let Some(Value::Array(targets)) = body.get_mut("targets") {
            let mut index = 0;
            targets.retain(|_| {
                index += 1;
                !unused.contains(&(index - 1))
            });
        }
        if let Some(effects) = body.get_mut("effects") {
            compact(effects, &unused);
        }
        repairs.push(Repair {
            class: RepairClass::TargetUnused,
            at: site.at.to_owned(),
            note: format!(
                "{} declared target(s) no effect uses and the text has no \"target\": dropped",
                unused.len()
            ),
        });
        return;
    }
    let [index] = unused[..] else {
        return;
    };
    let wired = body
        .get("targets")
        .and_then(Value::as_array)
        .and_then(|targets| targets.get(index))
        .is_some_and(of_objects);
    let Some(Value::Array(effects)) = body.get_mut("effects") else {
        return;
    };
    let mut slots = own_slots(effects);
    if let (true, [slot]) = (wired, &mut slots[..]) {
        **slot = json!({ "Target": index });
        repairs.push(Repair {
            class: RepairClass::TargetUnused,
            at: site.at.to_owned(),
            note: format!("target {index} is declared from the text and the one effect that names its own object takes it: wired"),
        });
    }
}
