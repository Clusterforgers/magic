//! A seeded deck of cards with miracle, cards that draw and discard, and lands of their colours, for fuzz games.

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_core::color::Color;
use mtg_engine::GameSetup;
use mtg_engine::format::Normal;
use mtg_engine::rng::GameRng;

use crate::cards::land;
use crate::corpus;

/// The corpus cards the miracle deck is made of besides lands: the keyword, and what draws and discards.
pub const MIRACLE_CARDS: &[&str] = &[
    "Sister Repentia",
    "Thunderous Wrath",
    "Vanishment",
    "Brainstorm",
    "Opt",
    "Rummaging Goblin",
    "Lightning Bolt",
];

/// A legal Normal deck of lands, four of each such card, derived entirely from `rng`.
pub fn miracle_deck(rng: &mut GameRng) -> Vec<Arc<CardDefinition>> {
    let colors = [Color::White, Color::Blue, Color::Red, Color::Black];
    let mut deck: Vec<Arc<CardDefinition>> = (0..32).map(|i| land(colors[i % 4])).collect();
    for card in MIRACLE_CARDS
        .iter()
        .filter_map(|name| corpus::card(name).ok())
    {
        deck.extend(std::iter::repeat_n(card, 4));
    }
    rng.shuffle(&mut deck);
    deck.truncate(60);
    deck
}

/// A normal game of `players` seats whose decks are [`miracle_deck`]s, fully determined by `seed`.
pub fn miracle_fuzz_setup(players: usize, seed: u64) -> GameSetup {
    let decks = (0u64..)
        .take(players)
        .map(|seat| miracle_deck(&mut GameRng::substream(seed, 60_000 + seat)))
        .collect();
    GameSetup::new(Arc::new(Normal), seed, decks)
}
