//! A group of one of two colors before its noun: "return a white or blue creature you control to its owner's hand". The
//! two colors are written as a subtype no card has ("W16amark creature") and the words are swapped.

use serde_json::{Value, json};

use super::w4d_parts::wordings;
use crate::ability::Ability;

/// The subtype that stands for the two colors while the ability is written.
const W16A_COLORS: &str = "W16amark";

/// Adds the wordings of an ability with one selector of either of two colors.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let Ok(mut value) = serde_json::to_value(ability) else {
        return;
    };
    let mut found = Vec::new();
    w16a_marked(&mut value, &mut found);
    let [(first, second)] = found.as_slice() else {
        return;
    };
    let mark = format!("a {W16A_COLORS} ");
    for text in wordings(value) {
        if text.matches(W16A_COLORS).count() == 1 && text.contains(&mark) {
            texts.push(text.replace(&mark, &format!("a {first} or {second} ")));
        }
    }
}

/// Swaps each `AnyOf` of exactly two colors in a list of predicates for the mark, collecting the colors in lower case.
fn w16a_marked(value: &mut Value, found: &mut Vec<(String, String)>) {
    match value {
        Value::Array(items) => {
            for item in items.iter_mut() {
                if let Some(colors) = w16a_two_colors(item) {
                    found.push(colors);
                    *item = json!({"Subtype": W16A_COLORS});
                } else {
                    w16a_marked(item, found);
                }
            }
        }
        Value::Object(map) => map.values_mut().for_each(|inner| w16a_marked(inner, found)),
        _ => {}
    }
}

/// The two colors of `{"AnyOf": [{"Color": A}, {"Color": B}]}`.
fn w16a_two_colors(pred: &Value) -> Option<(String, String)> {
    let [first, second] = pred.get("AnyOf")?.as_array()?.as_slice() else {
        return None;
    };
    let color = |pred: &Value| {
        pred.get("Color")
            .and_then(Value::as_str)
            .map(str::to_lowercase)
    };
    Some((color(first)?, color(second)?))
}
