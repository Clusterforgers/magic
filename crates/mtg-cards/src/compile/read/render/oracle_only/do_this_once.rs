//! The limit of a trigger worded as an instruction: "Whenever you cast a noncreature spell, you may return target
//! creature card ... Do this only once each turn." The renderers write "Then this ability triggers only once each turn."

use crate::ability::{Ability, Limit};

/// Adds the wordings of a trigger limited to once each turn that say "Do this only once each turn".
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let Ability::Triggered {
        limit: Some(Limit::OncePerTurn),
        ..
    } = ability
    else {
        return;
    };
    let said: Vec<String> = texts
        .iter()
        .filter(|text| text.contains(" Then this ability triggers only once each turn."))
        .map(|text| {
            text.replacen(
                " Then this ability triggers only once each turn.",
                " Do this only once each turn.",
                1,
            )
        })
        .collect();
    texts.extend(said);
}
