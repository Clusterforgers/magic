//! Station for the fuzz games: the corpus Spacecraft with their station brackets, beside plain creatures to tap for
//! them and the lands that cast them.

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_core::color::Color;
use mtg_engine::GameSetup;
use mtg_engine::format::Normal;
use mtg_engine::rng::GameRng;

use crate::cards::{creature, land};
use crate::corpus;

/// The corpus Spacecraft: a station keyword and brackets of keywords, abilities and a lord.
pub const STATION_CARDS: &[&str] = &[
    "Galvanizing Sawship",
    "Wurmwall Sweeper",
    "Uthros Scanship",
    "Wedgelight Rammer",
    "Rescue Skiff",
    "Susurian Dirgecraft",
    "Atmospheric Greenhouse",
    "Debris Field Crusher",
    "Lumen-Class Frigate",
    "Uthros Research Craft",
    "The Seriema",
    "Hearthhull, the Worldseed",
];

/// A legal Normal deck of lands, the corpus Spacecraft and plain creatures of every power, derived entirely from `rng`.
pub fn station_deck(rng: &mut GameRng) -> Vec<Arc<CardDefinition>> {
    let colors = [
        Color::White,
        Color::Blue,
        Color::Black,
        Color::Red,
        Color::Green,
    ];
    let mut deck: Vec<Arc<CardDefinition>> = (0..22).map(|i| land(colors[i % 5])).collect();
    for name in STATION_CARDS {
        if let Ok(card) = corpus::card(name) {
            deck.extend(std::iter::repeat_n(card, 2));
        }
    }
    for slot in 0..14 {
        let power = i32::try_from(1 + rng.index(4)).unwrap_or(1);
        let name = format!("Helper {slot} {power}/{power}");
        deck.push(creature(&name, 1, &[colors[rng.index(5)]], power, power));
    }
    rng.shuffle(&mut deck);
    deck.truncate(60);
    deck
}

/// A normal game of `players` seats whose decks are [`station_deck`]s, fully determined by `seed`.
pub fn station_fuzz_setup(players: usize, seed: u64) -> GameSetup {
    let decks = (0u64..)
        .take(players)
        .map(|seat| station_deck(&mut GameRng::substream(seed, 27_000 + seat)))
        .collect();
    GameSetup::new(Arc::new(Normal), seed, decks)
}
