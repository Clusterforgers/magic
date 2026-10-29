//! A count of the cards a target player has in a zone: "~ deals damage to target player equal to the number of cards
//! in that player's hand." The count is written as the player's own ("your hand") and the words are swapped.

use serde_json::{Value, json};

use super::w4d_parts::wordings;
use crate::ability::Ability;

/// The zones of a count and their words after "your".
const W13A_ZONES: [(&str, &str); 2] = [("Hand", "hand"), ("Graveyard", "graveyard")];

/// Adds the wordings of an ability whose one count of a target player's cards in a zone names that player again.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let Ok(mut value) = serde_json::to_value(ability) else {
        return;
    };
    let own = json!({"OwnedBy": "You"});
    let one_target = value
        .as_object()
        .and_then(|map| map.values().next())
        .and_then(|inner| inner.get("targets"))
        .and_then(Value::as_array)
        .is_some_and(|targets| targets.len() == 1);
    if !one_target || w13a_contains(&value, &own) {
        return;
    }
    let mut zones = Vec::new();
    w13a_as_own(&mut value, &mut zones);
    let [zone] = zones.as_slice() else {
        return;
    };
    let Some((_, word)) = W13A_ZONES.iter().find(|(name, _)| name == zone) else {
        return;
    };
    let yours = format!("your {word}");
    for text in wordings(value) {
        let Some((before, _)) = text.split_once(&yours) else {
            continue;
        };
        if text.matches(&yours).count() == 1 && before.contains("target player") {
            texts.push(text.replace(&yours, &format!("that player's {word}")));
        }
    }
}

/// Whether `part` occurs anywhere in `value`.
fn w13a_contains(value: &Value, part: &Value) -> bool {
    value == part
        || match value {
            Value::Object(map) => map.values().any(|inner| w13a_contains(inner, part)),
            Value::Array(items) => items.iter().any(|inner| w13a_contains(inner, part)),
            _ => false,
        }
}

/// Makes each count of the cards a target owns in a zone a count of the controller's own, collecting the zones.
fn w13a_as_own(value: &mut Value, zones: &mut Vec<String>) {
    match value {
        Value::Object(map) => {
            if let Some(Value::Array(preds)) = map.get_mut("Count") {
                let zone = preds
                    .iter()
                    .find_map(|pred| pred.get("In").and_then(Value::as_str).map(str::to_owned));
                let owner = preds.iter_mut().find(|pred| {
                    pred.get("OwnedBy")
                        .is_some_and(|who| who.get("Target").is_some())
                });
                if let (Some(zone), Some(owner)) = (zone, owner) {
                    *owner = json!({"OwnedBy": "You"});
                    zones.push(zone);
                }
            }
            map.values_mut().for_each(|inner| w13a_as_own(inner, zones));
        }
        Value::Array(items) => items.iter_mut().for_each(|inner| w13a_as_own(inner, zones)),
        _ => {}
    }
}
