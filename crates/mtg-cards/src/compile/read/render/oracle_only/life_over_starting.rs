//! A static ability that works while the life total is a number of points over the starting life total: "As long as you
//! have at least 7 life more than your starting life total, creatures you control get +2/+2."

use super::super::variants;
use super::twin_json::{lowered, twin};
use crate::ability::Ability;

/// The points over the starting life total that the condition asks for.
fn margin(ability: &Ability) -> Option<i64> {
    let value = serde_json::to_value(ability).ok()?;
    let compare = value.get("Static")?.get("condition")?.get("Compare")?;
    if compare.get("left")?.get("Life")? != "You" || compare.get("op")? != "Ge" {
        return None;
    }
    let sum = compare.get("right")?.get("Sum")?.as_array()?;
    match sum.as_slice() {
        [start, points] if start.get("StartingLife")? == "You" => points.as_i64(),
        _ => None,
    }
}

/// Adds the wordings of a static ability under that condition.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let Some(points) = margin(ability) else {
        return;
    };
    let plain = twin(ability, |value| {
        value
            .get_mut("Static")?
            .as_object_mut()?
            .remove("condition")
            .map(|_| ())
    });
    let Some(plain) = plain else {
        return;
    };
    let said: Vec<String> = variants(&plain)
        .iter()
        .map(|text| {
            format!(
                "As long as you have at least {points} life more than your starting life total, {}",
                lowered(text)
            )
        })
        .collect();
    texts.extend(said);
}
