//! A choice of modes where one mode may be chosen more than once: "Choose three. You may choose the same mode more than
//! once." The renderers word the choice of the same number of different modes.

use super::super::variants;
use super::twin_json::{inner_mut, twin};
use crate::ability::Ability;

/// Adds the wordings of a choice with a repeated mode.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let plain = twin(ability, |value| {
        let modal = inner_mut(value)?
            .get_mut("effects")?
            .as_array_mut()?
            .first_mut()?
            .get_mut("Modal")?
            .as_object_mut()?;
        (modal.get("repeat")? == true).then_some(())?;
        modal.remove("repeat").map(|_| ())
    });
    let Some(plain) = plain else {
        return;
    };
    let said: Vec<String> = variants(&plain)
        .iter()
        .filter_map(|text| text.split_once(" \u{2014}"))
        .filter(|(head, _)| head.starts_with("Choose "))
        .map(|(head, rest)| format!("{head}. You may choose the same mode more than once.{rest}"))
        .collect();
    texts.extend(said);
}
