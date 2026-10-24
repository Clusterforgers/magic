//! An exile that returns at once: "Exile target creature you control, then return it to the battlefield under its owner's
//! control." The renderers write the exile and the return as two sentences.

use super::super::variants;
use super::twin_json::{inner_mut, twin};
use crate::ability::Ability;
use serde_json::{Value, json};

/// How the return words the controller of the card, by the destination of the move.
fn control(to: &Value) -> Option<&'static str> {
    if *to == json!({"Battlefield": {}}) {
        Some(" under its owner's control")
    } else if *to == json!({"Battlefield": {"under": "You"}}) {
        Some(" under your control")
    } else {
        None
    }
}

/// Adds the wordings of an exile followed by the return of the exiled card.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let mut under = None;
    let plain = twin(ability, |value| {
        let effects = inner_mut(value)?.get_mut("effects")?.as_array_mut()?;
        let [exile, back] = effects.as_slice() else {
            return None;
        };
        let bound = exile.get("Exile")?.get("as")?;
        let moved = back.get("Move")?;
        (moved.get("what")?.get("Bound")? == bound).then_some(())?;
        under = control(moved.get("to")?);
        under?;
        effects.pop().map(|_| ())
    });
    let (Some(plain), Some(under)) = (plain, under) else {
        return;
    };
    let said: Vec<String> = variants(&plain)
        .iter()
        .filter(|text| text.contains("xile "))
        .filter_map(|text| text.strip_suffix('.'))
        .map(|text| format!("{text}, then return it to the battlefield{under}."))
        .collect();
    texts.extend(said);
}
