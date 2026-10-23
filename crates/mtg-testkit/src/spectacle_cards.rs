//! A seeded deck of the corpus cards with spectacle, afterlife and horsemanship, with lands of their colours and creatures
//! to attack with, block and die, for fuzz games.

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_core::color::Color;
use mtg_engine::GameSetup;
use mtg_engine::format::Normal;
use mtg_engine::rng::GameRng;

use crate::cards::{creature, land};
use crate::corpus;

/// The corpus cards with spectacle, afterlife or horsemanship.
pub const SPECTACLE_CARDS: [&str; 12] = [
    "Light Up the Stage",
    "Skewer the Critics",
    "Rafter Demon",
    "Spikewheel Acrobat",
    "Hackrobat",
    "Spawn of Mayhem",
    "Ministrant of Obligation",
    "Syndicate Messenger",
    "Orzhov Enforcer",
    "Debtors' Transport",
    "Shu Cavalry",
    "Lu Xun, Scholar General",
];

/// A legal Normal deck of lands, three of each such card and creatures to attack and block with, derived entirely from `rng`.
pub fn spectacle_deck(rng: &mut GameRng) -> Vec<Arc<CardDefinition>> {
    let colors = [Color::Black, Color::Red, Color::White];
    let mut deck: Vec<Arc<CardDefinition>> = (0..24).map(|i| land(colors[i % 3])).collect();
    for card in SPECTACLE_CARDS
        .iter()
        .filter_map(|name| corpus::card(name).ok())
    {
        deck.extend(std::iter::repeat_n(card, 3));
    }
    let mut slot = 0;
    while deck.len() < 60 {
        let power = i32::try_from(1 + rng.index(3)).unwrap_or(1);
        let name = format!("Spectacle Fodder {slot} {power}/{power}");
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

/// A normal game of `players` seats whose decks are [`spectacle_deck`]s, fully determined by `seed`.
pub fn spectacle_fuzz_setup(players: usize, seed: u64) -> GameSetup {
    let decks = (0u64..)
        .take(players)
        .map(|seat| spectacle_deck(&mut GameRng::substream(seed, 36_000 + seat)))
        .collect();
    GameSetup::new(Arc::new(Normal), seed, decks)
}
