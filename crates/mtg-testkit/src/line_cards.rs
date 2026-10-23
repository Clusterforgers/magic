//! A seeded deck of the corpus cards with graft and afflict, with lands of their colours and creatures to attack with,
//! block and enter, for fuzz games.

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_core::color::Color;
use mtg_engine::GameSetup;
use mtg_engine::format::Normal;
use mtg_engine::rng::GameRng;

use crate::cards::{creature, land};
use crate::corpus;

/// The corpus cards with graft or afflict.
pub const LINE_CARDS: [&str; 4] = [
    "Aquastrand Spider",
    "Llanowar Reborn",
    "Plaxcaster Frogling",
    "Neheb, the Eternal",
];

/// A legal Normal deck of lands, three of each such card and creatures to attack and block with, derived entirely from `rng`.
pub fn line_deck(rng: &mut GameRng) -> Vec<Arc<CardDefinition>> {
    let colors = [Color::Green, Color::Blue, Color::Red];
    let mut deck: Vec<Arc<CardDefinition>> = (0..24).map(|i| land(colors[i % 3])).collect();
    for card in LINE_CARDS.iter().filter_map(|name| corpus::card(name).ok()) {
        deck.extend(std::iter::repeat_n(card, 3));
    }
    let mut slot = 0;
    while deck.len() < 60 {
        let power = i32::try_from(1 + rng.index(3)).unwrap_or(1);
        let name = format!("Line Fodder {slot} {power}/{power}");
        deck.extend(std::iter::repeat_n(
            creature(&name, 1, &[colors[rng.index(3)]], power, power),
            2,
        ));
        slot += 1;
    }
    rng.shuffle(&mut deck);
    deck.truncate(60);
    deck
}

/// A normal game of `players` seats whose decks are [`line_deck`]s, fully determined by `seed`.
pub fn line_fuzz_setup(players: usize, seed: u64) -> GameSetup {
    let decks = (0u64..)
        .take(players)
        .map(|seat| line_deck(&mut GameRng::substream(seed, 41_000 + seat)))
        .collect();
    GameSetup::new(Arc::new(Normal), seed, decks)
}
