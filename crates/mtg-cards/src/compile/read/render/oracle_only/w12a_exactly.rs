//! A count equal to a number: "Activate only if you have exactly seven cards in hand." The ability is written with the
//! count at least the number ("seven or more cards") and the words are swapped.

use serde_json::{Value, json};

use super::super::super::amounts::count_word;
use super::w4d_parts::wordings;
use crate::ability::Ability;

/// Adds the wordings of an ability with one comparison of a count equal to a number.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let Ok(mut value) = serde_json::to_value(ability) else {
        return;
    };
    let mut numbers = Vec::new();
    w12a_at_least(&mut value, &mut numbers);
    let [number] = numbers.as_slice() else {
        return;
    };
    let mut said = vec![(format!("{number} or more"), format!("exactly {number}"))];
    if let Some(word) = u32::try_from(*number).ok().and_then(count_word) {
        said.push((format!("{word} or more"), format!("exactly {word}")));
    }
    for text in wordings(value) {
        for (from, to) in &said {
            if text.matches(from.as_str()).count() == 1 {
                texts.push(text.replace(from.as_str(), to));
            }
        }
    }
}

/// Turns each comparison of a count equal to a number into one of at least that number, collecting the numbers.
fn w12a_at_least(value: &mut Value, numbers: &mut Vec<u64>) {
    match value {
        Value::Object(map) => {
            let count = map.get("left").and_then(|left| left.get("Count")).is_some();
            let number = map.get("right").and_then(Value::as_u64);
            if let (true, Some(number), Some(op)) = (count, number, map.get_mut("op"))
                && *op == "Eq"
                && number > 0
            {
                *op = json!("Ge");
                numbers.push(number);
            }
            map.values_mut()
                .for_each(|inner| w12a_at_least(inner, numbers));
        }
        Value::Array(items) => items
            .iter_mut()
            .for_each(|inner| w12a_at_least(inner, numbers)),
        _ => {}
    }
}
