//! A step that depends on the spell having been foretold: "Destroy target creature. If this spell was foretold, scry 2."
//! The renderers know the same condition for a kicked spell.

use super::twin_variants;
use crate::ability::Ability;

/// Adds the wordings of a spell with a step "if this spell was foretold".
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let said: Vec<String> = twin_variants(ability, r#"{"CastWith":"Foretold"}"#, r#""Kicked""#)
        .iter()
        .filter(|text| {
            text.contains("if this spell was kicked") || text.contains("If this spell was kicked")
        })
        .map(|text| text.replace(" was kicked", " was foretold"))
        .collect();
    texts.extend(said);
}
