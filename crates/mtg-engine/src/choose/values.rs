//! The lists a choice of a value (a type, a card name) offers, read from the cards of the game.

use mtg_core::card::{CardType, LAND_TYPES};

use super::ChoiceOption;
use crate::object::GameObject;
use crate::state::GameState;

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

/// The creature types of the creature cards in the game, sorted, each once.
pub(crate) fn creature_types(state: &GameState) -> Vec<ChoiceOption> {
    let mut types: Vec<String> = state
        .objects()
        .filter(|o| o.card().is_type(CardType::Creature))
        .flat_map(|o| o.card().subtypes.iter().map(|s| s.0.clone()))
        .collect();
    types.sort();
    types.dedup();
    types.into_iter().map(ChoiceOption::Label).collect()
}

/// The names of the cards in the game (every zone, tokens left out) that `keep` accepts, sorted, each once. The engine knows no cards
/// beyond the ones in the game.
pub(crate) fn card_names(
    state: &GameState,
    mut keep: impl FnMut(&GameObject) -> bool,
) -> Vec<ChoiceOption> {
    let mut names: Vec<String> = state
        .objects()
        .filter(|o| o.kind().is_card() && keep(o))
        .map(|o| o.card().name.clone())
        .collect();
    names.sort();
    names.dedup();
    names.into_iter().map(ChoiceOption::Label).collect()
}
