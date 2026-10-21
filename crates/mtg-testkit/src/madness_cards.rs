//! A seeded deck of cards with madness and suspend, discard outlets and lands of their colours, for fuzz games.

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_core::color::Color;
use mtg_engine::GameSetup;
use mtg_engine::format::Normal;
use mtg_engine::rng::GameRng;

use crate::cards::land;
use crate::corpus;

/// The corpus cards the madness and suspend deck is made of besides lands: the keywords and what discards.
pub const MADNESS_CARDS: [&str; 10] = [
    "Fiery Temper",
    "Basking Rootwalla",
    "Arrogant Wurm",
    "Rift Bolt",
    "Ancestral Vision",
    "Durkwood Baloth",
    "Faithless Looting",
    "Rummaging Goblin",
    "Lightning Bolt",
    "Brainstorm",
];

/// A legal Normal deck of lands, three of each such card, derived entirely from `rng`.
pub fn madness_deck(rng: &mut GameRng) -> Vec<Arc<CardDefinition>> {
    let colors = [Color::Red, Color::Green, Color::Blue];
    let mut deck: Vec<Arc<CardDefinition>> = (0..30).map(|i| land(colors[i % 3])).collect();
    for card in MADNESS_CARDS
        .iter()
        .filter_map(|name| corpus::card(name).ok())
    {
        deck.extend(std::iter::repeat_n(card, 3));
    }
    rng.shuffle(&mut deck);
    deck.truncate(60);
    deck
}

/// A normal game of `players` seats whose decks are [`madness_deck`]s, fully determined by `seed`.
pub fn madness_fuzz_setup(players: usize, seed: u64) -> GameSetup {
    let decks = (0u64..)
        .take(players)
        .map(|seat| madness_deck(&mut GameRng::substream(seed, 50_000 + seat)))
        .collect();
    GameSetup::new(Arc::new(Normal), seed, decks)
}
