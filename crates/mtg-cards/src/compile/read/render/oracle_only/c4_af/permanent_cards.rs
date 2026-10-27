//! A permanent card with a mana value at most the source's power: "return up to one target permanent card with mana value
//! less than or equal to ~'s power from your graveyard to the battlefield." The renderers list the five permanent types
//! and write the limit as "X or less, where X is its power".

use super::text::json;
use crate::ability::Ability;

/// The five permanent types a card in a graveyard can have, as the renderers list them.
const TYPES: &str = "artifact, creature, enchantment, land, or planeswalker card";

/// The limit as the renderers write it and as the card writes it.
const LIMIT: (&str, &str) = (
    "with mana value X or less",
    "with mana value less than or equal to ~'s power",
);

/// Adds the wordings of a card limited by the power of the source.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let limited =
        r#""Compare":{"stat":"ManaValue","op":"Le","to":{"Stat":{"of":"This","stat":"Power"}}}"#;
    if !json(ability).contains(limited) {
        return;
    }
    let said: Vec<String> = texts
        .iter()
        .filter(|text| text.contains(TYPES) && text.contains(LIMIT.0))
        .map(|text| {
            text.replacen(TYPES, "permanent card", 1)
                .replacen(LIMIT.0, LIMIT.1, 1)
                .replacen(", where X is its power", "", 1)
        })
        .collect();
    texts.extend(said);
}
