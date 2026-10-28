//! The controller of a target named by its card type: "Destroy target land. ~ deals 2 damage to that land's
//! controller." The ability is written with a target permanent, "that permanent's controller", and both are named by
//! the type.

use serde_json::{Value, json};

use super::w4d_parts::wordings;
use crate::ability::Ability;

/// Adds the wordings of an ability with one target of one card type whose controller a later step names.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let Ok(mut value) = serde_json::to_value(ability) else {
        return;
    };
    let Some(kind) = w8c_target_kind(&mut value) else {
        return;
    };
    let named = format!("that {kind}'s controller");
    let target = format!("target {kind}");
    for text in wordings(value) {
        if text.matches("target permanent").count() == 1
            && text.contains("that permanent's controller")
        {
            texts.push(
                text.replace("that permanent's controller", &named)
                    .replacen("target permanent", &target, 1),
            );
        }
    }
}

/// The card type of the one target, in lower case, when it is a permanent named by that type alone; the target of the
/// JSON becomes any permanent.
fn w8c_target_kind(value: &mut Value) -> Option<String> {
    let inner = value.as_object_mut()?.values_mut().next()?;
    let [target] = inner.get_mut("targets")?.as_array_mut()?.as_mut_slice() else {
        return None;
    };
    let what = target.get_mut("what")?;
    let [pred] = what.get("Object")?.as_array()?.as_slice() else {
        return None;
    };
    let kind = pred.get("Type").and_then(Value::as_str)?;
    let kind = matches!(
        kind,
        "Land" | "Creature" | "Artifact" | "Enchantment" | "Planeswalker" | "Battle"
    )
    .then(|| kind.to_lowercase())?;
    *what = json!({"Object": []});
    Some(kind)
}
