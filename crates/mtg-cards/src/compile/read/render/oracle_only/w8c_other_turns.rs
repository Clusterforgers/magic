//! A condition that it is not the controller's turn, said as the oracle does: "During turns other than yours, spells
//! you cast cost {1} less to cast." The ability is written with the condition that it is the controller's turn.

use serde_json::json;

use super::w4d_parts::{swapped, wordings};
use crate::ability::Ability;

/// The wordings of the condition that it is your turn and of the one that it is not.
const W8C_TURNS: [(&str, &str); 2] = [
    ("If it's your turn, ", "During turns other than yours, "),
    (" if it's your turn", " during turns other than yours"),
];

/// Adds the wordings of an ability under the condition that it is not your turn.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let Ok(mut value) = serde_json::to_value(ability) else {
        return;
    };
    let said = value.to_string();
    if said.matches("\"YourTurn\"").count() != said.matches("{\"Not\":\"YourTurn\"}").count() {
        return;
    }
    if !swapped(&mut value, &json!({"Not": "YourTurn"}), &json!("YourTurn")) {
        return;
    }
    for text in wordings(value) {
        for (yours, others) in W8C_TURNS {
            if text.matches(yours).count() == 1 {
                texts.push(text.replacen(yours, others, 1));
            }
        }
    }
}
