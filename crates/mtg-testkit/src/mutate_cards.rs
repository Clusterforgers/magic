//! A seeded deck of the corpus creatures with mutate, with lands of their colours and creatures to mutate onto, to block
//! and to fill the graveyards, for fuzz games.

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_core::color::Color;
use mtg_engine::GameSetup;
use mtg_engine::format::Normal;
use mtg_engine::rng::GameRng;

use crate::cards::{creature, land};
use crate::corpus;

/// The corpus creatures with mutate that the engine plays whole.
pub const MUTATE_CARDS: [&str; 9] = [
    "Cavern Whisperer",
    "Chittering Harvester",
    "Dreamtail Heron",
    "Gemrazer",
    "Glowstone Recluse",
    "Migratory Greathorn",
    "Pouncing Shoreshark",
    "Sawtusk Demolisher",
    "Trumpeting Gnarr",
];

/// A legal Normal deck of lands, four of each such card and cheap creatures to mutate onto, derived entirely from `rng`.
pub fn mutate_deck(rng: &mut GameRng) -> Vec<Arc<CardDefinition>> {
    let colors = [Color::Black, Color::Green, Color::Red, Color::Blue];
    let mut deck: Vec<Arc<CardDefinition>> = (0..24).map(|i| land(colors[i % 4])).collect();
    for card in MUTATE_CARDS
        .iter()
        .filter_map(|name| corpus::card(name).ok())
    {
        deck.extend(std::iter::repeat_n(card, 3));
    }
    // Removal now and then takes the creature a mutating spell targets while the spell is on the stack.
    if let Ok(removal) = corpus::card("Doom Blade") {
        deck.extend(std::iter::repeat_n(removal, 4));
    }
    // Cheap creatures fill the deck up: they are what the mutating spells merge with, and the bodies that attack and block.
    let mut slot = 0;
    while deck.len() < 60 {
        let power = i32::try_from(1 + rng.index(3)).unwrap_or(1);
        let name = format!("Mutate Fodder {slot} {power}/{power}");
        let card = creature(&name, 1, &[colors[rng.index(4)]], power, power);
        deck.extend(std::iter::repeat_n(card, 4));
        slot += 1;
    }
    rng.shuffle(&mut deck);
    deck.truncate(60);
    deck
}

/// A normal game of `players` seats whose decks are [`mutate_deck`]s, fully determined by `seed`.
pub fn mutate_fuzz_setup(players: usize, seed: u64) -> GameSetup {
    let decks = (0u64..)
        .take(players)
        .map(|seat| mutate_deck(&mut GameRng::substream(seed, 34_000 + seat)))
        .collect();
    GameSetup::new(Arc::new(Normal), seed, decks)
}
