//! A seeded deck of the corpus creatures with encore, with lands of their colours and creatures to fill the graveyards and
//! to block, for fuzz games.

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_core::color::Color;
use mtg_engine::GameSetup;
use mtg_engine::format::Normal;
use mtg_engine::rng::GameRng;

use crate::cards::{creature, land};
use crate::corpus;

/// The corpus creatures with encore.
pub const ENCORE_CARDS: [&str; 5] = [
    "Impulsive Pilferer",
    "Kinsbaile Courier",
    "Exquisite Huntmaster",
    "Trove Tracker",
    "Briarblade Adept",
];

/// A legal Normal deck of lands, three of each such card and creatures, derived entirely from `rng`.
pub fn encore_deck(rng: &mut GameRng) -> Vec<Arc<CardDefinition>> {
    let colors = [Color::White, Color::Blue, Color::Red, Color::Black];
    let mut deck: Vec<Arc<CardDefinition>> = (0..26).map(|i| land(colors[i % 4])).collect();
    for card in ENCORE_CARDS
        .iter()
        .filter_map(|name| corpus::card(name).ok())
    {
        deck.extend(std::iter::repeat_n(card, 3));
    }
    // Creatures fill the deck up: they attack and block, and the ones that die fill the graveyards.
    for slot in 0..7 {
        let power = i32::try_from(1 + rng.index(3)).unwrap_or(1);
        let name = format!("Encore Fodder {slot} {power}/{power}");
        let card = creature(&name, 1, &[colors[rng.index(4)]], power, power);
        deck.extend(std::iter::repeat_n(card, 3));
    }
    rng.shuffle(&mut deck);
    deck.truncate(60);
    deck
}

/// A normal game of `players` seats whose decks are [`encore_deck`]s, fully determined by `seed`.
pub fn encore_fuzz_setup(players: usize, seed: u64) -> GameSetup {
    let decks = (0u64..)
        .take(players)
        .map(|seat| encore_deck(&mut GameRng::substream(seed, 34_000 + seat)))
        .collect();
    GameSetup::new(Arc::new(Normal), seed, decks)
}
