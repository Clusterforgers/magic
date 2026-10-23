//! A seeded deck of cards that double counters and reveal until a card matches: Kalonian Hydra, Primordial Hydra, Big
//! Mother Mouser, Vorel of the Hull Clade and Genesis Storm, with lands of their colours and creatures of every cost for
//! the reveal to find, for fuzz games.

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_core::color::Color;
use mtg_engine::GameSetup;
use mtg_engine::format::Normal;
use mtg_engine::rng::GameRng;

use crate::cards::{creature, land};
use crate::corpus;

/// The corpus cards the doubling deck is made of besides lands and creatures.
pub const MULTIPLY_CARDS: &[&str] = &[
    "Kalonian Hydra",
    "Primordial Hydra",
    "Big Mother Mouser",
    "Vorel of the Hull Clade",
    "Genesis Storm",
];

/// A legal Normal deck of lands, three of each such card and creatures of value 1 to 5, derived entirely from `rng`.
pub fn multiply_deck(rng: &mut GameRng) -> Vec<Arc<CardDefinition>> {
    let colors = [Color::Green, Color::Blue];
    let mut deck: Vec<Arc<CardDefinition>> = (0..30).map(|i| land(colors[i % 2])).collect();
    for card in MULTIPLY_CARDS
        .iter()
        .filter_map(|name| corpus::card(name).ok())
    {
        deck.extend(std::iter::repeat_n(card, 3));
    }
    for value in 1u32..=5 {
        let name = format!("Doubling Fodder {value}");
        let power = i32::try_from(1 + value % 3).unwrap_or(1);
        deck.extend(std::iter::repeat_n(
            creature(&name, value, &[], power, 2),
            3,
        ));
    }
    rng.shuffle(&mut deck);
    deck.truncate(60);
    deck
}

/// A normal game of `players` seats whose decks are [`multiply_deck`]s, fully determined by `seed`.
pub fn multiply_fuzz_setup(players: usize, seed: u64) -> GameSetup {
    let decks = (0u64..)
        .take(players)
        .map(|seat| multiply_deck(&mut GameRng::substream(seed, 41_000 + seat)))
        .collect();
    GameSetup::new(Arc::new(Normal), seed, decks)
}
