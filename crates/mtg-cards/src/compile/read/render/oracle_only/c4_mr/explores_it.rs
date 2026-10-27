//! The permanent of an event explores: "Whenever a creature you control enters, it explores." The renderers write the
//! explore of the source.

use super::twin_variants;
use crate::ability::Ability;

/// Adds the wordings of a trigger whose creature explores.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let said: Vec<String> = twin_variants(
        ability,
        r#""Explore":{"what":"Triggering"}"#,
        r#""Explore":{"what":"This"}"#,
    )
    .iter()
    .filter(|text| text.contains(", ~ explores."))
    .map(|text| text.replacen(", ~ explores.", ", it explores.", 1))
    .collect();
    texts.extend(said);
}
