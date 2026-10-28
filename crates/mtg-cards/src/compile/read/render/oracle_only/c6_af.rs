//! Wordings of the sixth sample of the names A to F that only the checker accepts: twins of an ability the renderers
//! write with the card's words put back, phrase swaps on the wordings they give, and an ability whose two steps each
//! act on a target of their own, worded as two sentences.

mod c6_af_split;
mod c6_af_twins;

use crate::ability::Ability;

/// Adds the wordings of this sample to `texts`; the twins also edit the wordings found before them.
pub(super) fn c6_af_add(ability: &Ability, texts: &mut Vec<String>) {
    c6_af_split::c6_af_add(ability, texts);
    c6_af_twins::c6_af_add(ability, texts);
}

/// The ability as JSON, which the guards search for the shape they want.
fn c6_af_json(ability: &Ability) -> String {
    serde_json::to_string(ability).unwrap_or_default()
}

/// The wordings the renderers give an ability.
fn c6_af_variants(ability: &Ability) -> Vec<String> {
    super::super::variants(ability)
}

/// The wordings the checker accepts for an ability, the writers of this sample included; only for a twin that lacks the
/// part its writer looks for, so that it never comes back to the same writer.
fn c6_af_written(ability: &Ability) -> Vec<String> {
    super::super::written_variants(ability)
}
