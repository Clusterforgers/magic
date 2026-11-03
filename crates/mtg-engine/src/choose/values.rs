//! The lists a choice of a value (a type, a card name) offers, read from the cards of the game.

use mtg_core::card::{CardType, LAND_TYPES};

use super::ChoiceOption;

/// Every card type a player may name.
pub(crate) const CARD_TYPES: [CardType; 9] = [
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

/// The card types, by name.
pub(crate) fn card_types() -> Vec<ChoiceOption> {
    CARD_TYPES
        .iter()
        .map(|kind| ChoiceOption::Label(format!("{kind:?}")))
        .collect()
}

/// The basic land types, by name.
pub(crate) fn land_types() -> Vec<ChoiceOption> {
    LAND_TYPES
        .iter()
        .map(|name| ChoiceOption::Label((*name).to_owned()))
        .collect()
}

/// Every creature type of the game, sorted: the whole list, so the options say nothing of the cards in hidden zones.
pub(crate) fn creature_types() -> Vec<ChoiceOption> {
    mtg_cards::compile::creature_types()
        .into_iter()
        .map(|name| ChoiceOption::Label(name.to_owned()))
        .collect()
}
