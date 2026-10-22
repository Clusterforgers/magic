//! A seeded deck of the corpus cards with dredge (Darkblast, Greater Mossdog, Shenanigans and Shambling Shell), with
//! lands of their colours, draw spells, burn and artifacts to fill the graveyards and to draw from the library, and
//! creatures to target and to die, for fuzz games.

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_core::color::Color;
use mtg_engine::GameSetup;
use mtg_engine::format::Normal;
use mtg_engine::rng::GameRng;

use crate::cards::{burn, creature, land, study};
use crate::corpus;

/// The corpus cards with dredge.
pub const DREDGE_CARDS: [&str; 4] = [
    "Darkblast",
    "Greater Mossdog",
    "Shenanigans",
    "Shambling Shell",
];

/// A legal Normal deck of lands, four of each card with dredge, artifacts for Shenanigans, draw spells, burn and
/// creatures, derived entirely from `rng`.
pub fn dredge_deck(rng: &mut GameRng) -> Vec<Arc<CardDefinition>> {
    let colors = [Color::Black, Color::Green, Color::Red, Color::Blue];
    let mut deck: Vec<Arc<CardDefinition>> = (0..24).map(|i| land(colors[i % 4])).collect();
    for card in DREDGE_CARDS
        .iter()
        .chain(["Lotus Petal", "Arcane Signet"].iter())
        .filter_map(|name| corpus::card(name).ok())
    {
        deck.extend(std::iter::repeat_n(card, 3));
    }
    deck.extend(std::iter::repeat_n(study("Dredge Study"), 4));
    deck.extend(std::iter::repeat_n(burn("Dredge Burn", 3), 3));
    // Creatures fill the deck up: they are the targets of the spells and the bodies that attack and block.
    let mut slot = 0;
    while deck.len() < 60 {
        let power = i32::try_from(1 + rng.index(3)).unwrap_or(1);
        let name = format!("Dredge Fodder {slot} {power}/{power}");
        let card = creature(&name, 1, &[colors[rng.index(4)]], power, power);
        deck.extend(std::iter::repeat_n(card, 2));
        slot += 1;
    }
    rng.shuffle(&mut deck);
    deck.truncate(60);
    deck
}

/// A normal game of `players` seats whose decks are [`dredge_deck`]s, fully determined by `seed`.
pub fn dredge_fuzz_setup(players: usize, seed: u64) -> GameSetup {
    let decks = (0u64..)
        .take(players)
        .map(|seat| dredge_deck(&mut GameRng::substream(seed, 33_000 + seat)))
        .collect();
    GameSetup::new(Arc::new(Normal), seed, decks)
}
