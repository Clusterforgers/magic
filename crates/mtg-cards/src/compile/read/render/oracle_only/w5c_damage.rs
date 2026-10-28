//! A damage trigger with a least amount and a source of any kind: "Whenever a source you control deals 5 or more damage
//! to a player, ...". The amount is the intervening "if" that compares the damage dealt with a number, and a source
//! selector with no kind of object reads "source".

use serde_json::{Value, json};

use super::w4d_parts::wordings;
use crate::ability::Ability;

/// Adds the wordings of a damage trigger whose intervening "if" asks for at least an amount of damage.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let Ok(mut value) = serde_json::to_value(ability) else {
        return;
    };
    let Some(inner) = value.get_mut("Triggered").and_then(Value::as_object_mut) else {
        return;
    };
    let Some(source) = inner
        .get("when")
        .and_then(|when| when.pointer("/DealsDamage/source"))
        .cloned()
    else {
        return;
    };
    let Some(least) = inner.get("if").and_then(w5c_least) else {
        return;
    };
    inner.remove("if");
    let any_kind = matches!(source.as_array().map(Vec::as_slice), Some([only]) if only.get("ControlledBy").is_some());
    for text in wordings(value) {
        let Some(text) = w5c_at_least(&text, least) else {
            continue;
        };
        if any_kind && text.contains("a permanent you control deals ") {
            texts.push(text.replacen(
                "a permanent you control deals ",
                "a source you control deals ",
                1,
            ));
        }
        texts.push(text);
    }
}

/// The least amount of damage of a condition that compares the damage dealt with a number.
fn w5c_least(condition: &Value) -> Option<i64> {
    let compare = condition.get("Compare")?;
    (compare.get("left")? == &json!("Triggering") && compare.get("op")? == &json!("Ge"))
        .then(|| compare.get("right")?.as_i64())
        .flatten()
}

/// The trigger head with "N or more" before the damage it deals.
fn w5c_at_least(text: &str, least: i64) -> Option<String> {
    let head = text.split_once(", ")?.0;
    for damage in ["deals damage ", "deals combat damage "] {
        if head.contains(damage) {
            let more = damage.replacen("deals ", &format!("deals {least} or more "), 1);
            return Some(text.replacen(damage, &more, 1));
        }
    }
    None
}
