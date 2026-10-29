//! A target that deals damage to itself: "Target creature deals damage to itself equal to its power." The ability is
//! written with the damage dealt to a second target of the same kind and the words of that target become "itself".

use serde_json::{Value, json};

use super::w4d_parts::wordings;
use crate::ability::Ability;

/// The words of damage with an amount, as the renderers write them.
const W13A_EQUAL: &str = "deals damage equal to ";

/// Adds the wordings of an ability with one target whose one damage effect has the target as source and recipient.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let Ok(mut value) = serde_json::to_value(ability) else {
        return;
    };
    let Some(inner) = value
        .as_object_mut()
        .and_then(|map| map.values_mut().next())
    else {
        return;
    };
    let Some([target]) = inner
        .get("targets")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
    else {
        return;
    };
    let target = target.clone();
    let first = json!({"Target": 0});
    let Some([effect]) = inner
        .get_mut("effects")
        .and_then(Value::as_array_mut)
        .map(Vec::as_mut_slice)
    else {
        return;
    };
    let Some(damage) = effect.get_mut("DealDamage") else {
        return;
    };
    if damage.get("source") != Some(&first) || damage.get("to") != Some(&first) {
        return;
    }
    damage["to"] = json!({"Target": 1});
    inner["targets"] = json!([target.clone(), target]);
    texts.extend(wordings(value).iter().filter_map(|text| w13a_said(text)));
}

/// "X deals damage equal to A to target creature." as "X deals damage to itself equal to A."
fn w13a_said(text: &str) -> Option<String> {
    let (lead, rest) = text.split_once(W13A_EQUAL)?;
    let (amount, after) = rest.split_once(" to target ")?;
    let (_, tail) = after.split_once('.')?;
    if amount.contains("target") {
        return None;
    }
    Some(format!(
        "{lead}deals damage to itself equal to {amount}.{tail}"
    ))
}
