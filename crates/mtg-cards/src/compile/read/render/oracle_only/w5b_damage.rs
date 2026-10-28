//! A trigger on damage of at least an amount: "Whenever a source you control deals 5 or more damage to a player, draw a
//! card." The intervening condition on the damage dealt is said in the event; a source without a card type is "a
//! source".

use serde_json::{Value, json};

use super::w4d_parts::wordings;
use super::w5b_parts::w5b_number;
use crate::ability::Ability;

/// Adds the wordings of a damage trigger whose condition is that the damage is at least an amount.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let Ok(mut value) = serde_json::to_value(ability) else {
        return;
    };
    let Some(inner) = value.get_mut("Triggered").and_then(Value::as_object_mut) else {
        return;
    };
    let Some(least) = inner
        .get("if")
        .and_then(|test| test.get("Compare"))
        .filter(|compare| compare.get("left") == Some(&json!("Triggering")))
        .filter(|compare| compare.get("op") == Some(&json!("Ge")))
        .and_then(|compare| compare.get("right"))
        .and_then(Value::as_u64)
    else {
        return;
    };
    let Some(source) = inner
        .get("when")
        .and_then(|when| when.get("DealsDamage"))
        .and_then(|damage| damage.get("source"))
        .and_then(Value::as_array)
    else {
        return;
    };
    let typeless = source.iter().all(|pred| {
        pred.get("Type").is_none() && pred.get("Subtype").is_none() && *pred != json!("This")
    });
    inner.remove("if");
    let digits = least.to_string();
    let amounts: Vec<&str> = [Some(digits.as_str()), w5b_number(&json!(least))]
        .into_iter()
        .flatten()
        .collect();
    for text in wordings(value) {
        let Some((event, effect)) = text.split_once(", ") else {
            continue;
        };
        if !event.contains(" deals damage to ") {
            continue;
        }
        for amount in &amounts {
            let event = event.replace(
                " deals damage to ",
                &format!(" deals {amount} or more damage to "),
            );
            texts.push(format!("{event}, {effect}"));
            if typeless {
                texts.push(format!(
                    "{}, {effect}",
                    event.replacen("a permanent ", "a source ", 1)
                ));
            }
        }
    }
}
