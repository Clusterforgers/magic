//! A seeded deck of the corpus creature cards with bestow, with removal that takes the hosts away and creatures to
//! enchant, for fuzz games.

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_core::color::Color;
use mtg_engine::GameSetup;
use mtg_engine::format::Normal;
use mtg_engine::rng::GameRng;

use crate::cards::{creature, land};
use crate::corpus;

/// The corpus cards with bestow.
pub const BESTOW_CARDS: [&str; 4] = [
    "Boon Satyr",
    "Celestial Archon",
    "Ghostblade Eidolon",
    "Chromanticore",
];

/// The corpus removal that kills the creatures the cards are bestowed on.
const REMOVAL: [&str; 2] = ["Doom Blade", "Lightning Bolt"];

/// A legal Normal deck of lands of the five colours, four of each bestow card (two Chromanticores), removal and creatures,
/// derived entirely from `rng`.
pub fn bestow_deck(rng: &mut GameRng) -> Vec<Arc<CardDefinition>> {
    let colors = [
        Color::Green,
        Color::White,
        Color::Black,
        Color::Red,
        Color::Blue,
    ];
    let mut deck: Vec<Arc<CardDefinition>> = (0..25).map(|i| land(colors[i % 5])).collect();
    for name in BESTOW_CARDS {
        let copies = if name == "Chromanticore" { 2 } else { 4 };
        if let Ok(card) = corpus::card(name) {
            deck.extend(std::iter::repeat_n(card, copies));
        }
    }
    for card in REMOVAL.iter().filter_map(|name| corpus::card(name).ok()) {
        deck.extend(std::iter::repeat_n(card, 4));
    }
    // Creatures fill the deck up: they are the hosts the cards are bestowed on and the targets of the removal.
    let mut slot = 0;
    while deck.len() < 60 {
        let power = i32::try_from(1 + rng.index(3)).unwrap_or(1);
        let name = format!("Bestow Fodder {slot} {power}/{power}");
        let card = creature(&name, 1, &[colors[rng.index(5)]], power, power);
        deck.extend(std::iter::repeat_n(card, 3));
        slot += 1;
    }
    rng.shuffle(&mut deck);
    deck.truncate(60);
    deck
}

/// A normal game of `players` seats whose decks are [`bestow_deck`]s, fully determined by `seed`.
pub fn bestow_fuzz_setup(players: usize, seed: u64) -> GameSetup {
    let decks = (0u64..)
        .take(players)
        .map(|seat| bestow_deck(&mut GameRng::substream(seed, 33_000 + seat)))
        .collect();
    GameSetup::new(Arc::new(Normal), seed, decks)
}
