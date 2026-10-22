//! A seeded deck of the corpus cards that are cast or activated from the graveyard or exiled to be cast later: Dregscape
//! Zombie, Kathari Screecher and Reconstructed Thopter (unearth), Glimpse of Freedom, Fruit of Tizerus and Underworld
//! Rage-Hound (escape), Staggershock and Artful Maneuver (rebound), with lands of their colours and creatures to fill the
//! graveyards, for fuzz games.

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_core::color::Color;
use mtg_engine::GameSetup;
use mtg_engine::format::Normal;
use mtg_engine::rng::GameRng;

use crate::cards::{creature, land};
use crate::corpus;

/// The corpus cards that come back from a graveyard or exile.
pub const GRAVEYARD_CARDS: [&str; 8] = [
    "Dregscape Zombie",
    "Kathari Screecher",
    "Reconstructed Thopter",
    "Glimpse of Freedom",
    "Fruit of Tizerus",
    "Underworld Rage-Hound",
    "Staggershock",
    "Artful Maneuver",
];

/// A legal Normal deck of lands, three of each such card and creatures that fill the graveyard, derived entirely from `rng`.
pub fn graveyard_deck(rng: &mut GameRng) -> Vec<Arc<CardDefinition>> {
    let colors = [Color::Black, Color::Blue, Color::Red];
    let mut deck: Vec<Arc<CardDefinition>> = (0..28).map(|i| land(colors[i % 3])).collect();
    for card in GRAVEYARD_CARDS
        .iter()
        .filter_map(|name| corpus::card(name).ok())
    {
        deck.extend(std::iter::repeat_n(card, 3));
    }
    for slot in 0..4 {
        let power = i32::try_from(1 + rng.index(3)).unwrap_or(1);
        let name = format!("Graveyard Fodder {slot} {power}/{power}");
        let card = creature(&name, 1, &[colors[rng.index(3)]], power, power);
        deck.extend(std::iter::repeat_n(card, 2));
    }
    rng.shuffle(&mut deck);
    deck.truncate(60);
    deck
}

/// A normal game of `players` seats whose decks are [`graveyard_deck`]s, fully determined by `seed`.
pub fn graveyard_fuzz_setup(players: usize, seed: u64) -> GameSetup {
    let decks = (0u64..)
        .take(players)
        .map(|seat| graveyard_deck(&mut GameRng::substream(seed, 31_000 + seat)))
        .collect();
    GameSetup::new(Arc::new(Normal), seed, decks)
}
