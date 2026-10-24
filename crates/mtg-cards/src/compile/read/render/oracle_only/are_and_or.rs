//! A group named by a subtype and tokens together: "Creatures you control that are Zombies and/or tokens get +1/+1 and
//! have flying." The renderers have no wording for the union.

use super::super::variants;
use super::twin_json::twin;
use crate::ability::Ability;
use serde_json::{Value, json};

/// The subtype of the union "Zombie and/or token" in the group's selector, taken out of the selector when `take` is set.
fn union_of(selector: &mut Vec<Value>, take: bool) -> Option<String> {
    let at = selector.iter().position(|pred| {
        pred.get("AnyOf")
            .and_then(Value::as_array)
            .is_some_and(|preds| preds.len() == 2 && preds.contains(&json!("Token")))
    })?;
    let name = selector[at]
        .get("AnyOf")?
        .as_array()?
        .iter()
        .find_map(|pred| pred.get("Subtype")?.as_str().map(str::to_owned))?;
    if take {
        selector.remove(at);
    }
    Some(name)
}

/// Adds the wordings of a static ability over the creatures that are of one subtype and/or tokens.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let mut name = None;
    let plain = twin(ability, |value| {
        let selector = value
            .get_mut("Static")?
            .get_mut("affected")?
            .get_mut("Each")?
            .as_array_mut()?;
        name = union_of(selector, true);
        name.as_ref().map(|_| ())
    });
    let (Some(plain), Some(name)) = (plain, name) else {
        return;
    };
    let said: Vec<String> = variants(&plain)
        .iter()
        .filter(|text| text.starts_with("Creatures you control "))
        .map(|text| {
            text.replacen(
                "Creatures you control ",
                &format!("Creatures you control that are {name}s and/or tokens "),
                1,
            )
        })
        .collect();
    texts.extend(said);
}
