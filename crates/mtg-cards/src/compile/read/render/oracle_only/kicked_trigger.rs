//! A permanent that was kicked is "it" in its own triggered ability: "When this creature enters, destroy all other
//! creatures you control. If it was kicked, destroy all other creatures instead." The renderers say "this spell".

use crate::ability::Ability;

/// Adds the wordings of a triggered ability that call the kicked card "it".
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    if !matches!(ability, Ability::Triggered { .. }) {
        return;
    }
    let found: Vec<String> = texts
        .iter()
        .filter(|text| text.to_lowercase().contains("if this spell was kicked"))
        .map(|text| {
            text.replace("if this spell was kicked", "if it was kicked")
                .replace("If this spell was kicked", "If it was kicked")
        })
        .collect();
    texts.extend(found);
}
