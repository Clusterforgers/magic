//! A seeded deck of the corpus cards that are kicked: Burst Lightning (kicker), Gatekeeper of Malakir (kicker and an
//! enters trigger that reads it) and Wolfbriar Elemental (multikicker and tokens for each payment), with lands of
//! their colours and creatures to aim them at, for fuzz games.

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_core::color::Color;
use mtg_engine::GameSetup;
use mtg_engine::format::Normal;
use mtg_engine::rng::GameRng;

use crate::cards::{creature, land};
use crate::corpus;

/// The corpus cards that pay optional costs.
pub const KICKED_CARDS: [&str; 3] = [
    "Burst Lightning",
    "Gatekeeper of Malakir",
    "Wolfbriar Elemental",
];

/// A legal Normal deck of lands, four of each kicked card and creatures to aim them at, derived entirely from `rng`.
pub fn kicker_deck(rng: &mut GameRng) -> Vec<Arc<CardDefinition>> {
    let colors = [Color::Red, Color::Black, Color::Green];
    let mut deck: Vec<Arc<CardDefinition>> = (0..27).map(|i| land(colors[i % 3])).collect();
    for card in KICKED_CARDS
        .iter()
        .filter_map(|name| corpus::card(name).ok())
    {
        deck.extend(std::iter::repeat_n(card, 4));
    }
    for slot in 0..12 {
        let power = i32::try_from(1 + rng.index(3)).unwrap_or(1);
        let name = format!("Kicker Fodder {slot} {power}/{power}");
        let card = creature(&name, 1, &[colors[rng.index(3)]], power, power);
        deck.extend(std::iter::repeat_n(card, 2));
    }
    rng.shuffle(&mut deck);
    deck.truncate(60);
    deck
}

/// A normal game of `players` seats whose decks are [`kicker_deck`]s, fully determined by `seed`.
pub fn kicker_fuzz_setup(players: usize, seed: u64) -> GameSetup {
    let decks = (0u64..)
        .take(players)
        .map(|seat| kicker_deck(&mut GameRng::substream(seed, 20_000 + seat)))
        .collect();
    GameSetup::new(Arc::new(Normal), seed, decks)
}
