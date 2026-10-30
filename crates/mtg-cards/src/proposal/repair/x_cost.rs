//! An `X` in a face that prints none (no `{X}` in the cost, no X in the text) is the number the words of the ability
//! state, when they state one.

use std::collections::BTreeSet;

use serde_json::Value;

use super::text::{ability_text, has_word};
use super::{Repair, RepairClass, Site, body_of};

/// The number words the text may use for an amount.
const WORDS: [&str; 10] = [
    "one", "two", "three", "four", "five", "six", "seven", "eight", "nine", "ten",
];

/// The distinct amounts a text states, as digits or as words; a symbol in braces is no amount.
fn amounts(text: &str) -> BTreeSet<i64> {
    let mut plain = String::with_capacity(text.len());
    let mut inside = false;
    for c in text.chars() {
        match c {
            '{' => inside = true,
            '}' => inside = false,
            _ if !inside => plain.push(c),
            _ => {}
        }
    }
    plain
        .split(|c: char| !c.is_alphanumeric())
        .filter_map(|word| {
            word.parse::<i64>().ok().or_else(|| {
                let at = WORDS
                    .iter()
                    .position(|name| name.eq_ignore_ascii_case(word))?;
                i64::try_from(at + 1).ok()
            })
        })
        .collect()
}

/// The fields of an effect that hold an amount.
const AMOUNTS: [&str; 4] = ["amount", "count", "power", "toughness"];

/// Replaces the `X` of every amount field of the effects by the number, and says how many it replaced. An `X` in an
/// event or a condition ("equal to the chosen number") is no amount the text states.
fn replace(value: &mut Value, number: i64) -> usize {
    match value {
        Value::Object(map) => map
            .iter_mut()
            .map(|(key, inner)| {
                if AMOUNTS.contains(&key.as_str()) && inner.as_str() == Some("X") {
                    *inner = Value::from(number);
                    1
                } else if ["when", "condition", "if"].contains(&key.as_str()) {
                    0
                } else {
                    replace(inner, number)
                }
            })
            .sum(),
        Value::Array(items) => items.iter_mut().map(|inner| replace(inner, number)).sum(),
        _ => 0,
    }
}

/// Whether the value has an `X` amount.
fn has_x(value: &Value) -> bool {
    match value {
        Value::String(word) => word == "X",
        Value::Object(map) => map.values().any(has_x),
        Value::Array(items) => items.iter().any(has_x),
        _ => false,
    }
}

/// Puts the number the words of the ability state where the file wrote an `X` the card does not have.
pub(super) fn repair(ability: &mut Value, site: &Site<'_>, repairs: &mut Vec<Repair>) {
    let face = site.face;
    if face.mana_cost.contains("{X}") || has_word(&face.oracle_text, "X") || !has_x(ability) {
        return;
    }
    let Some((name, _)) = body_of(ability) else {
        return;
    };
    let Some(text) = ability_text(face, name, site.nth, site.count) else {
        return;
    };
    let numbers = amounts(&text);
    let mut stated = numbers.iter();
    let (Some(&number), None) = (stated.next(), stated.next()) else {
        return;
    };
    let mut repaired = ability.clone();
    let replaced = replace(&mut repaired, number);
    // Every X must be one the text settles: an X elsewhere stays invalid and nothing is changed.
    if replaced == 0 || has_x(&repaired) {
        return;
    }
    *ability = repaired;
    repairs.push(Repair {
        class: RepairClass::InventedX,
        at: site.at.to_owned(),
        note: format!("the card has no X: {replaced} X amount(s) are the {number} the text states"),
    });
}
