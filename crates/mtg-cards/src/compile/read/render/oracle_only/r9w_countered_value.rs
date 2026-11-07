//! The mana value of a countered spell, kept before the spell leaves the stack: "Counter target spell. Draw cards equal
//! to that spell's mana value.", "... Create X Treasure tokens, where X is that spell's mana value." The data counters
//! the target, remembers its mana value and reads the number back; the ability is written with the mana value of the
//! target in place of the number read back, and "its mana value" is said "that spell's mana value".

use serde_json::{Value, json};

use super::w4d_parts::{parts, with_effects};
use crate::ability::Ability;

/// Whether the value holds this exact JSON anywhere.
fn holds(value: &Value, wanted: &Value) -> bool {
    value == wanted
        || match value {
            Value::Object(map) => map.values().any(|child| holds(child, wanted)),
            Value::Array(items) => items.iter().any(|child| holds(child, wanted)),
            _ => false,
        }
}

/// The value with every copy of `from` replaced by `into`.
fn replaced(value: &Value, from: &Value, into: &Value) -> Value {
    if value == from {
        return into.clone();
    }
    match value {
        Value::Object(map) => Value::Object(
            map.iter()
                .map(|(key, child)| (key.clone(), replaced(child, from, into)))
                .collect(),
        ),
        Value::Array(items) => Value::Array(
            items
                .iter()
                .map(|child| replaced(child, from, into))
                .collect(),
        ),
        other => other.clone(),
    }
}

/// Adds the wordings of an ability that counters its target spell and then reads the spell's remembered mana value.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let Some((value, effects)) = parts(ability) else {
        return;
    };
    let target = json!({"Target": 0});
    let [counter, remember, rest @ ..] = effects.as_slice() else {
        return;
    };
    if *counter != json!({"Counter": {"what": target}}) {
        return;
    }
    let Some(slot) = remember
        .get("RememberValue")
        .filter(|kept| {
            kept.get("value") == Some(&json!({"Stat": {"of": target, "stat": "ManaValue"}}))
        })
        .and_then(|kept| kept.get("slot"))
    else {
        return;
    };
    let read = json!({"Remembered": {"on": "This", "slot": slot}});
    let worth = json!({"Stat": {"of": target, "stat": "ManaValue"}});
    // The number is read back by one step, and the kept slot is read nowhere else.
    if rest.iter().filter(|effect| holds(effect, &read)).count() != 1 {
        return;
    }
    let mut steps = vec![counter.clone()];
    steps.extend(rest.iter().map(|effect| replaced(effect, &read, &worth)));
    if holds(&json!(steps), slot) {
        return;
    }
    for text in with_effects(&value, &steps) {
        if text.matches("its mana value").count() == 1 {
            texts.push(text.replacen("its mana value", "that spell's mana value", 1));
        }
    }
}
