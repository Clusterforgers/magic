//! The defending player as the one who acts: "Whenever this creature attacks, defending player discards a card." The
//! ability is written with each opponent in that place and the words are swapped.

use serde_json::{Value, json};

use super::w4d_parts::{swapped, wordings};
use crate::ability::Ability;

/// Adds the wordings of an ability in which the defending player loses life, discards or does another action.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let Ok(mut value) = serde_json::to_value(ability) else {
        return;
    };
    let opponents = json!("Opponents");
    if w16a_mentions(&value, &opponents) || !swapped(&mut value, &json!("Defending"), &opponents) {
        return;
    }
    for text in wordings(value) {
        if text.matches("each opponent").count() == 1 && !text.contains("opponents") {
            texts.push(text.replace("each opponent", "defending player"));
        }
    }
}

/// Whether the JSON holds the value anywhere.
fn w16a_mentions(value: &Value, wanted: &Value) -> bool {
    value == wanted
        || match value {
            Value::Array(items) => items.iter().any(|item| w16a_mentions(item, wanted)),
            Value::Object(map) => map.values().any(|item| w16a_mentions(item, wanted)),
            _ => false,
        }
}
