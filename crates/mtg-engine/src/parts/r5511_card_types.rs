//! Exiling cards from the payer's graveyard with enough card types among them as a cost ("Exile any number of other cards
//! from your graveyard with four or more card types among them", Nethergoyf's escape cost).

use mtg_core::card::CardType;
use mtg_core::ids::ObjectId;

use crate::characteristics::characteristics;
use crate::state::GameState;

/// The card types a card can have (CR 300.1).
const R5511_TYPES: [CardType; 9] = [
    CardType::Artifact,
    CardType::Battle,
    CardType::Creature,
    CardType::Enchantment,
    CardType::Instant,
    CardType::Kindred,
    CardType::Land,
    CardType::Planeswalker,
    CardType::Sorcery,
];

/// How many card types there are among `objects`, each counted once.
pub(super) fn r5511_types_among(state: &GameState, objects: &[ObjectId]) -> u32 {
    let seen: Vec<_> = objects
        .iter()
        .filter_map(|object| characteristics(state, *object))
        .collect();
    let among = R5511_TYPES
        .iter()
        .filter(|card_type| seen.iter().any(|chars| chars.types.contains(**card_type)))
        .count();
    u32::try_from(among).unwrap_or(u32::MAX)
}

/// "Exile cards from your graveyard with four or more card types among them."
pub(super) fn r5511_text(count: usize) -> String {
    format!("Exile cards from your graveyard with {count} or more card types among them.")
}
