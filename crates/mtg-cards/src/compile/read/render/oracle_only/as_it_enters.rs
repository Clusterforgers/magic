//! A choice made as the permanent enters, worded after another sentence about the entry: "This land enters tapped. As it
//! enters, choose a color."

use crate::ability::Ability;
use crate::trigger::ReplaceBy;

/// Adds the wordings of an entry choice that call the permanent "it".
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let Ability::Replacement {
        by: ReplaceBy::Choose(_),
        ..
    } = ability
    else {
        return;
    };
    let said: Vec<String> = texts
        .iter()
        .filter(|text| text.starts_with("As ~ enters, choose "))
        .map(|text| text.replacen("As ~ enters, ", "As it enters, ", 1))
        .collect();
    texts.extend(said);
}
