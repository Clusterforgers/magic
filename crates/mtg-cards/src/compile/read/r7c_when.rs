//! A condition held as a number (r7c): "the number that is 1 when C holds, else 0" compared with 1 or 0 is C itself or
//! its negation. A rule no writer writes is written again with each such comparison replaced by the condition, so
//! "If you're the monarch, ... instead" reads back whichever way the rule holds the test.

use serde_json::{Value, json};

use crate::ability::Ability;

/// The condition a comparison of the number of a condition stands for: the condition when the number is 1, its
/// negation when it is 0. Any other comparison, or numbers other than 1 and 0, stays.
fn plain(value: &Value) -> Option<Value> {
    let compare = value.get("Compare")?;
    let when = compare.get("left")?.get("When")?;
    (when.get("then")? == &json!(1) && when.get("otherwise")? == &json!(0)).then_some(())?;
    let condition = when.get("condition")?.clone();
    let holds = match (compare.get("op")?.as_str()?, compare.get("right")?.as_i64()?) {
        ("Ge" | "Eq", 1) | ("Gt", 0) => true,
        ("Eq" | "Le", 0) | ("Lt", 1) => false,
        _ => return None,
    };
    Some(if holds {
        condition
    } else {
        json!({ "Not": condition })
    })
}

/// Replaces each comparison of the number of a condition in the value; whether one was replaced.
fn replace(value: &mut Value) -> bool {
    if let Some(condition) = plain(value) {
        *value = condition;
        return true;
    }
    match value {
        Value::Object(map) => map.values_mut().fold(false, |found, v| replace(v) | found),
        Value::Array(list) => list.iter_mut().fold(false, |found, v| replace(v) | found),
        _ => false,
    }
}

/// The ability with each comparison of the number of a condition replaced by the condition; `None` when it has none.
pub(super) fn r7c_unwrapped(ability: &Ability) -> Option<Ability> {
    let mut value = serde_json::to_value(ability).ok()?;
    replace(&mut value).then_some(())?;
    serde_json::from_value(value).ok()
}
