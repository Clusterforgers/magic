//! A seeded deck of the corpus cards with scavenge, transmute, awaken, gravestorm and sunburst, with lands of their colours
//! and creatures to fill the graveyards, for fuzz games.

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_core::color::Color;
use mtg_engine::GameSetup;
use mtg_engine::format::Normal;
use mtg_engine::rng::GameRng;

use crate::cards::{creature, land};
use crate::corpus;

/// The corpus cards the deck holds, two of each.
pub const RECURSION_CARDS: [&str; 17] = [
    "Drudge Beetle",
    "Dreg Mangler",
    "Zanikev Locust",
    "Deadbridge Goliath",
    "Dimir Infiltrator",
    "Drift of Phantasms",
    "Muddle the Mixture",
    "Clutch of the Undercity",
    "Clutch of Currents",
    "Coastal Discovery",
    "Mire's Malice",
    "Ruinous Path",
    "Rising Miasma",
    "Follow the Bodies",
    "Ominous Harvest",
    "Pentad Prism",
    "Suntouched Myr",
];

/// A legal Normal deck of lands, two of each such card and fodder creatures, derived entirely from `rng`.
pub fn recursion_deck(rng: &mut GameRng) -> Vec<Arc<CardDefinition>> {
    let colors = [Color::Blue, Color::Black, Color::Green];
    let mut deck: Vec<Arc<CardDefinition>> = (0..24).map(|i| land(colors[i % 3])).collect();
    for card in RECURSION_CARDS
        .iter()
        .filter_map(|name| corpus::card(name).ok())
    {
        deck.extend(std::iter::repeat_n(card, 2));
    }
    // Creatures that attack and block, so permanents reach the graveyards.
    for slot in 0..2 {
        let power = i32::try_from(1 + rng.index(3)).unwrap_or(1);
        let name = format!("Recursion Fodder {slot} {power}/{power}");
        deck.push(creature(&name, 1, &[colors[rng.index(3)]], power, power));
    }
    rng.shuffle(&mut deck);
    deck.truncate(60);
    deck
}

/// A normal game of `players` seats whose decks are [`recursion_deck`]s, fully determined by `seed`.
pub fn recursion_fuzz_setup(players: usize, seed: u64) -> GameSetup {
    let decks = (0u64..)
        .take(players)
        .map(|seat| recursion_deck(&mut GameRng::substream(seed, 35_000 + seat)))
        .collect();
    GameSetup::new(Arc::new(Normal), seed, decks)
}
