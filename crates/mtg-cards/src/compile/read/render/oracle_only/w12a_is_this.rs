//! The source named through a selector, `{"Is": "This"}` in place of `"This"` ("Whenever this creature attacks, ..."),
//! and a sacrifice of the permanents that are the source in place of `SacrificeSelf` ("Sacrifice this creature: ..."):
//! the ability is written with the plain forms, which match the same object.

use serde_json::{Value, json};

use super::w4d_parts::{swapped, wordings};
use crate::ability::Ability;

/// Adds the wordings of an ability that names its source through a selector.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let Ok(mut value) = serde_json::to_value(ability) else {
        return;
    };
    let this = json!("This");
    let selected = json!({"Is": "This"});
    let mut changed = w12a_contains(&value, &selected) && swapped(&mut value, &this, &this);
    changed |= w12a_plain_sacrifice(&mut value);
    if changed {
        texts.extend(wordings(value));
    }
}

/// Whether the JSON holds the value anywhere.
fn w12a_contains(value: &Value, wanted: &Value) -> bool {
    value == wanted
        || match value {
            Value::Object(map) => map.values().any(|inner| w12a_contains(inner, wanted)),
            Value::Array(items) => items.iter().any(|inner| w12a_contains(inner, wanted)),
            _ => false,
        }
}

/// Replaces each cost item that sacrifices one permanent that is the source by `SacrificeSelf`; whether one was.
fn w12a_plain_sacrifice(value: &mut Value) -> bool {
    let source = json!({"Sacrifice": {"filter": ["This"]}});
    match value {
        Value::Object(map) => {
            let Some(cost) = map.get_mut("cost").and_then(Value::as_array_mut) else {
                return map
                    .values_mut()
                    .fold(false, |any, inner| w12a_plain_sacrifice(inner) | any);
            };
            let mut any = false;
            for item in cost.iter_mut().filter(|item| **item == source) {
                *item = json!("SacrificeSelf");
                any = true;
            }
            any
        }
        Value::Array(items) => items
            .iter_mut()
            .fold(false, |any, inner| w12a_plain_sacrifice(inner) | any),
        _ => false,
    }
}
