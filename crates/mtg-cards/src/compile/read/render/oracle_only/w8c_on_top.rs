//! An object put on top of its owner's library: "When this creature dies, you may put it on top of its owner's
//! library." The ability is written with the bottom of the library and the place is swapped.

use serde_json::{Value, json};

use super::w4d_parts::{swapped, wordings};
use crate::ability::Ability;

/// Adds the wordings of an ability that puts an object on top of its owner's library.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let Ok(mut value) = serde_json::to_value(ability) else {
        return;
    };
    let bottom = json!({"Library": {"place": "Bottom"}});
    if swapped(&mut value.clone(), &bottom, &bottom) {
        return;
    }
    if !swapped(&mut value, &json!({"Library": {"place": "Top"}}), &bottom) {
        return;
    }
    let may = w8c_unwrapped_may(&mut value);
    let said: Vec<String> = wordings(value)
        .iter()
        .filter(|text| text.matches(" on the bottom of ").count() == 1)
        .map(|text| text.replacen(" on the bottom of ", " on top of ", 1))
        .filter_map(|text| {
            if !may {
                return Some(text);
            }
            let (head, rest) = text.split_once(", put ")?;
            Some(format!("{head}, you may put {rest}"))
        })
        .collect();
    texts.extend(said);
}

/// Replaces a lone "you may" step of a trigger by the step it offers; whether it did.
fn w8c_unwrapped_may(value: &mut Value) -> bool {
    let Some(effects) = value
        .pointer_mut("/Triggered/effects")
        .and_then(Value::as_array_mut)
    else {
        return false;
    };
    let [effect] = effects.as_mut_slice() else {
        return false;
    };
    let Some(Value::Array(inner)) = effect.pointer("/May/effects") else {
        return false;
    };
    let [step] = inner.as_slice() else {
        return false;
    };
    if effect
        .pointer("/May/who")
        .is_some_and(|who| *who != json!("You"))
    {
        return false;
    }
    *effect = step.clone();
    true
}
