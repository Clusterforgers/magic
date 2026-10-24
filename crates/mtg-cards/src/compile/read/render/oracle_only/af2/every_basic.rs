//! Permanents that are every basic land type: "Lands you control are every basic land type in addition to their other
//! types." The renderers list the five types one by one.

use super::super::super::{Form, capitalize, phrase_text};
use crate::ability::Ability;
use crate::modify::Modification;
use crate::refs::Ref;
use mtg_core::card::Subtype;

const BASIC: [&str; 5] = ["Plains", "Island", "Swamp", "Mountain", "Forest"];

/// Adds the wording of a static ability that gives a group the five basic land types.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let Ability::Static {
        affected: Ref::Each(group),
        mods,
        condition: None,
        defining: false,
        ..
    } = ability
    else {
        return;
    };
    let given: Vec<&str> = mods
        .iter()
        .filter_map(|change| match change {
            Modification::AddSubtype(Subtype(name)) => Some(name.as_str()),
            _ => None,
        })
        .collect();
    if given != BASIC || mods.len() != BASIC.len() {
        return;
    }
    if let Some(group) = phrase_text(group, Form::All) {
        texts.push(format!(
            "{} are every basic land type in addition to their other types.",
            capitalize(&group)
        ));
    }
}
