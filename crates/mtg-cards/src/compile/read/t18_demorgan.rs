//! A translator spells "a legendary artifact" as "not (not artifact or not legendary)": in a selector's list of
//! conditions (all of which hold) that is the two conditions themselves.

use serde_json::Value;

use crate::ability::Ability;

/// The two conditions of `{"Not":{"AnyOf":[{"Not":a},{"Not":b}]}}`, or none for any other element.
fn both(element: &Value) -> Option<Vec<Value>> {
    let any = element.get("Not")?.get("AnyOf")?.as_array()?;
    (any.len() >= 2).then_some(())?;
    any.iter()
        .map(|item| item.get("Not").cloned())
        .collect::<Option<Vec<_>>>()
}

/// Flattens the lists of conditions under `Each` and `Object`; whether anything changed.
fn flatten(value: &mut Value) -> bool {
    let mut changed = false;
    match value {
        Value::Array(items) => {
            for item in items.iter_mut() {
                changed |= flatten(item);
            }
        }
        Value::Object(map) => {
            for (key, item) in map.iter_mut() {
                changed |= flatten(item);
                if key != "Each" && key != "Object" {
                    continue;
                }
                let Some(list) = item.as_array_mut() else {
                    continue;
                };
                if !list.iter().any(|element| both(element).is_some()) {
                    continue;
                }
                let old = std::mem::take(list);
                for element in old {
                    match both(&element) {
                        Some(parts) => list.extend(parts),
                        None => list.push(element),
                    }
                }
                changed = true;
            }
        }
        _ => {}
    }
    changed
}

/// The ability with every "not (not a or not b)" in a list of conditions written as "a" and "b" (`None` when it has none).
pub(super) fn t18_flattened(ability: &Ability) -> Option<Ability> {
    let mut value = serde_json::to_value(ability).ok()?;
    flatten(&mut value).then_some(())?;
    serde_json::from_value(value).ok()
}
