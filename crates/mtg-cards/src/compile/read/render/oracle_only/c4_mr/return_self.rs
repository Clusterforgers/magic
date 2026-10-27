//! A cost that returns the card to its owner's hand: "{2}, Return ~ to its owner's hand: Copy target instant or sorcery
//! spell you control." The renderers know the cost of a sacrifice.

use super::twin_variants;
use crate::ability::Ability;

/// The return of the source as a cost, and the sacrifice of the source the renderers write.
const RETURN: &str = r#"{"ReturnToHand":{"filter":["This"]}}"#;
const SACRIFICE: &str = r#""SacrificeSelf""#;

/// Adds the wordings of an activated ability that returns its source to its owner's hand as a cost.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let said: Vec<String> = twin_variants(ability, RETURN, SACRIFICE)
        .iter()
        .filter(|text| text.contains(", Sacrifice ~:"))
        .map(|text| text.replacen(", Sacrifice ~:", ", Return ~ to its owner's hand:", 1))
        .collect();
    texts.extend(said);
}
