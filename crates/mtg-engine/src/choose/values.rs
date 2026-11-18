//! The lists a choice of a value (a type, a card name) offers, read from the cards of the game.

use mtg_core::card::{CardType, CardTypes, LAND_TYPES};

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

/// The card types of the set, by name, in the order of [`CARD_TYPES`].
pub(crate) fn card_types_of(set: CardTypes) -> Vec<ChoiceOption> {
    CARD_TYPES
        .iter()
        .filter(|kind| set.contains(**kind))
        .map(|kind| ChoiceOption::Label(format!("{kind:?}")))
        .collect()
}

/// The five basic land types (CR 205.3i), by name.
pub(crate) fn basic_land_types() -> Vec<ChoiceOption> {
    ["Plains", "Island", "Swamp", "Mountain", "Forest"]
        .into_iter()
        .map(|name| ChoiceOption::Label(name.to_owned()))
        .collect()
}

/// The land types, by name.
pub(crate) fn land_types() -> Vec<ChoiceOption> {
    LAND_TYPES
        .iter()
        .map(|name| ChoiceOption::Label((*name).to_owned()))
        .collect()
}

/// The planeswalker types of the game (CR 205.3j), sorted.
pub(crate) const R6700_PLANESWALKER_TYPES: [&str; 79] = [
    "Ajani",
    "Aminatou",
    "Angrath",
    "Arlinn",
    "Ashiok",
    "Bahamut",
    "Basri",
    "Bolas",
    "Calix",
    "Chandra",
    "Comet",
    "Dack",
    "Dakkon",
    "Daretti",
    "Davriel",
    "Dihada",
    "Domri",
    "Dovin",
    "Ellywick",
    "Elminster",
    "Elspeth",
    "Estrid",
    "Freyalise",
    "Garruk",
    "Gideon",
    "Grist",
    "Guff",
    "Huatli",
    "Jace",
    "Jared",
    "Jaya",
    "Jeska",
    "Kaito",
    "Karn",
    "Kasmina",
    "Kaya",
    "Kiora",
    "Koth",
    "Liliana",
    "Lolth",
    "Lukka",
    "Minsc",
    "Mordenkainen",
    "Nahiri",
    "Narset",
    "Niko",
    "Nissa",
    "Nixilis",
    "Oko",
    "Quintorius",
    "Ral",
    "Rowan",
    "Saheeli",
    "Samut",
    "Sarkhan",
    "Serra",
    "Sivitri",
    "Sorin",
    "Szat",
    "Tamiyo",
    "Tasha",
    "Teferi",
    "Teyo",
    "Tezzeret",
    "Tibalt",
    "Tyvar",
    "Ugin",
    "Urza",
    "Venser",
    "Vivien",
    "Vraska",
    "Vronos",
    "Will",
    "Windgrace",
    "Wrenn",
    "Xenagos",
    "Yanggu",
    "Yanling",
    "Zariel",
];

/// Every planeswalker type of the game (CR 205.3j), sorted.
pub(crate) fn r6700_planeswalker_types() -> Vec<ChoiceOption> {
    R6700_PLANESWALKER_TYPES
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
