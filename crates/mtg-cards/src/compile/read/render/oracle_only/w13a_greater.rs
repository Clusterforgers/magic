//! A count greater than a number: "If you control a Giant, draw a card." with the count more than zero. The renderers
//! word a count of at least a number, so the comparison is turned into one of at least the next number.

use serde_json::{Value, json};

use super::w4d_parts::wordings;
use crate::ability::Ability;

/// Adds the wordings of an ability whose comparisons of a count with "greater than" are said as "at least" one more.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let Ok(mut value) = serde_json::to_value(ability) else {
        return;
    };
    if w13a_at_least(&mut value) {
        texts.extend(wordings(value));
    }
}

/// Turns each comparison of a count greater than a number into one of at least the next number; whether one was.
fn w13a_at_least(value: &mut Value) -> bool {
    match value {
        Value::Object(map) => {
            let mut changed = false;
            let count = map.get("left").and_then(|left| left.get("Count")).is_some();
            let number = map.get("right").and_then(Value::as_u64);
            if let (true, Some(number), Some(op)) = (count, number, map.get_mut("op"))
                && *op == "Gt"
            {
                *op = json!("Ge");
                map.insert("right".to_owned(), json!(number + 1));
                changed = true;
            }
            map.values_mut()
                .fold(changed, |changed, inner| w13a_at_least(inner) || changed)
        }
        Value::Array(items) => {
            items
                .iter_mut()
                .map(w13a_at_least)
                .filter(|changed| *changed)
                .count()
                > 0
        }
        _ => false,
    }
}
