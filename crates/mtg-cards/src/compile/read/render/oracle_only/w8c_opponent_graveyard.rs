//! A card an opponent owns put into a graveyard, said by whose graveyard: "Whenever an artifact is put into an
//! opponent's graveyard from the battlefield, you may draw a card." The trigger is written without the owner.

use serde_json::{Value, json};

use super::w4d_parts::wordings;
use crate::ability::Ability;

/// Adds the wordings of a trigger on a card an opponent owns put into a graveyard.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let Ok(mut value) = serde_json::to_value(ability) else {
        return;
    };
    let Some(change) = value.pointer_mut("/Triggered/when/ZoneChange") else {
        return;
    };
    if change.get("to") != Some(&json!("Graveyard")) {
        return;
    }
    let Some(Value::Array(what)) = change.get_mut("what") else {
        return;
    };
    let owned = json!({"OwnedBy": "Opponents"});
    let before = what.len();
    what.retain(|pred| *pred != owned);
    if what.len() + 1 != before || what.is_empty() {
        return;
    }
    for text in wordings(value) {
        for (plain, owned) in [
            (
                "is put into a graveyard",
                "is put into an opponent's graveyard",
            ),
            (
                "are put into a graveyard",
                "are put into an opponent's graveyard",
            ),
        ] {
            if text.matches(plain).count() == 1 {
                texts.push(text.replacen(plain, owned, 1));
            }
        }
    }
}
