//! A trigger of a card in the command zone: "At the beginning of your upkeep, if ~ is in the command zone, you gain 2
//! life." The renderers do not word the zone the card must be in.

use super::twin_variants;
use crate::ability::Ability;

/// Adds the wordings of a step trigger that works while the card is in the command zone.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let said: Vec<String> = twin_variants(ability, r#""from":"Command","#, "")
        .iter()
        .filter(|text| text.starts_with("At the beginning of "))
        .filter_map(|text| {
            let (head, rest) = text.split_once(", ")?;
            Some(format!("{head}, if ~ is in the command zone, {rest}"))
        })
        .collect();
    texts.extend(said);
}
