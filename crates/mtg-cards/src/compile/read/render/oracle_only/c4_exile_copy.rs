//! A card exiled and a token that is a copy of it: "Exile target artifact or creature card from your graveyard. Create a
//! token that's a copy of it." The renderers word the exile alone; and a pair of card types the oracle joins with
//! "and/or" where they write "or".

use serde_json::json;

use super::super::variants;
use super::twin_json::{inner_mut, twin};
use crate::ability::Ability;

/// Adds the wording of an exile of the target followed by a token that copies it.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let exile = json!({"Exile": {"what": {"Target": 0}}});
    let copy = json!({"CreateToken": {"token": {"CopyOf": {"of": {"Target": 0}}}}});
    let Some(plain) = twin(ability, |value| {
        let effects = inner_mut(value)?.get_mut("effects")?.as_array_mut()?;
        let shaped = effects.len() == 2 && effects[0] == exile && effects[1] == copy;
        shaped.then(|| effects.truncate(1))
    }) else {
        return;
    };
    let said: Vec<String> = variants(&plain)
        .iter()
        .filter_map(|text| text.strip_suffix('.'))
        .map(|text| format!("{text}. Create a token that's a copy of it."))
        .collect();
    texts.extend(said);
}

/// Adds the wordings that join artifact and creature cards with "and/or".
pub(super) fn add_and_or(texts: &mut Vec<String>) {
    const OR: &str = "artifact or creature card";
    const AND_OR: &str = "artifact and/or creature card";
    let said: Vec<String> = texts
        .iter()
        .filter(|text| text.contains(OR))
        .map(|text| text.replace(OR, AND_OR))
        .collect();
    texts.extend(said);
}
