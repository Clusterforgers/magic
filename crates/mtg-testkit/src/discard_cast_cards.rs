//! A seeded deck of the corpus cards cast from a graveyard for a discard (Radical Idea, Direct Current and Chemister's
//! Insight with jump-start, Flame Jab and Syphon Life with retrace) and the creatures that come back as Zombie tokens from it
//! (Sacred Cat, Tah-Crop Skirmisher and Aven Initiate with embalm, Adorned Pouncer and Proven Combatant with eternalize),
//! with lands of their colours and creatures to fill the graveyards, for fuzz games.

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_core::color::Color;
use mtg_engine::GameSetup;
use mtg_engine::format::Normal;
use mtg_engine::rng::GameRng;

use crate::cards::{creature, land};
use crate::corpus;

/// The corpus cards that are cast from a graveyard with a discard or embalmed and eternalized from it.
pub const DISCARD_CAST_CARDS: [&str; 10] = [
    "Radical Idea",
    "Direct Current",
    "Chemister's Insight",
    "Flame Jab",
    "Syphon Life",
    "Sacred Cat",
    "Tah-Crop Skirmisher",
    "Aven Initiate",
    "Adorned Pouncer",
    "Proven Combatant",
];

/// A legal Normal deck of lands, two of each such card and creatures that fill the graveyard, derived entirely from `rng`.
pub fn discard_cast_deck(rng: &mut GameRng) -> Vec<Arc<CardDefinition>> {
    let colors = [Color::White, Color::Blue, Color::Red, Color::Black];
    let mut deck: Vec<Arc<CardDefinition>> = (0..26).map(|i| land(colors[i % 4])).collect();
    for card in DISCARD_CAST_CARDS
        .iter()
        .filter_map(|name| corpus::card(name).ok())
    {
        deck.extend(std::iter::repeat_n(card, 2));
    }
    for slot in 0..7 {
        let power = i32::try_from(1 + rng.index(3)).unwrap_or(1);
        let name = format!("Discard Fodder {slot} {power}/{power}");
        let card = creature(&name, 1, &[colors[rng.index(4)]], power, power);
        deck.extend(std::iter::repeat_n(card, 2));
    }
    rng.shuffle(&mut deck);
    deck.truncate(60);
    deck
}

/// A normal game of `players` seats whose decks are [`discard_cast_deck`]s, fully determined by `seed`.
pub fn discard_cast_fuzz_setup(players: usize, seed: u64) -> GameSetup {
    let decks = (0u64..)
        .take(players)
        .map(|seat| discard_cast_deck(&mut GameRng::substream(seed, 32_000 + seat)))
        .collect();
    GameSetup::new(Arc::new(Normal), seed, decks)
}
