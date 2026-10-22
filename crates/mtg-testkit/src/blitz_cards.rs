//! A seeded deck of the corpus creatures with blitz, with lands of their colours and creatures to target, block and fill
//! the graveyards, for fuzz games.

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_core::color::Color;
use mtg_engine::GameSetup;
use mtg_engine::format::Normal;
use mtg_engine::rng::GameRng;

use crate::cards::{creature, land};
use crate::corpus;

/// The corpus creatures with blitz.
pub const BLITZ_CARDS: [&str; 5] = [
    "Riveteers Requisitioner",
    "Night Clubber",
    "Caldaia Strongarm",
    "Mayhem Patrol",
    "Girder Goons",
];

/// A legal Normal deck of lands, four of each such card and creatures, derived entirely from `rng`.
pub fn blitz_deck(rng: &mut GameRng) -> Vec<Arc<CardDefinition>> {
    let colors = [Color::Black, Color::Green, Color::Red, Color::Blue];
    let mut deck: Vec<Arc<CardDefinition>> = (0..26).map(|i| land(colors[i % 4])).collect();
    for card in BLITZ_CARDS
        .iter()
        .filter_map(|name| corpus::card(name).ok())
    {
        deck.extend(std::iter::repeat_n(card, 4));
    }
    // Creatures fill the deck up: they are the targets of the counters and the bodies that attack and block.
    let mut slot = 0;
    while deck.len() < 60 {
        let power = i32::try_from(1 + rng.index(3)).unwrap_or(1);
        let name = format!("Blitz Fodder {slot} {power}/{power}");
        let card = creature(&name, 1, &[colors[rng.index(4)]], power, power);
        deck.extend(std::iter::repeat_n(card, 3));
        slot += 1;
    }
    rng.shuffle(&mut deck);
    deck.truncate(60);
    deck
}

/// A normal game of `players` seats whose decks are [`blitz_deck`]s, fully determined by `seed`.
pub fn blitz_fuzz_setup(players: usize, seed: u64) -> GameSetup {
    let decks = (0u64..)
        .take(players)
        .map(|seat| blitz_deck(&mut GameRng::substream(seed, 33_000 + seat)))
        .collect();
    GameSetup::new(Arc::new(Normal), seed, decks)
}
