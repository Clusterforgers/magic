//! Planeswalkers the compiler reads (Garruk, Primal Hunter, Freyalise, Llanowar's Fury) beside two the table already
//! plays and spells that exile themselves as they resolve, with the deck and games built from them.

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_core::color::Color;
use mtg_engine::GameSetup;
use mtg_engine::format::Normal;
use mtg_engine::rng::GameRng;

use crate::cards::{creature, land};
use crate::corpus::{self, CorpusError};

/// Lands in a walker deck: three colours in turn.
const LANDS: usize = 26;

/// The corpus cards of the walker deck: planeswalkers read by the compiler and spells that exile themselves.
pub const WALKER_CARDS: &[&str] = &[
    "Garruk, Primal Hunter",
    "Freyalise, Llanowar's Fury",
    "Jace Beleren",
    "Garruk Wildspeaker",
    "Seeds of Renewal",
    "Teferi's Protection",
];

/// A legal Normal deck in green, blue and white of the cards above, two of each, with creatures to protect the
/// planeswalkers and to fill the graveyards.
///
/// It has its own generator, so the other decks and the games seeded from them stay as they were.
pub fn walker_deck(rng: &mut GameRng) -> Result<Vec<Arc<CardDefinition>>, CorpusError> {
    let colors = [Color::Green, Color::Blue, Color::White];
    let mut deck: Vec<Arc<CardDefinition>> = (0..LANDS).map(|i| land(colors[i % 3])).collect();
    for &name in WALKER_CARDS {
        deck.extend(std::iter::repeat_n(corpus::card(name)?, 2));
    }
    deck.extend(std::iter::repeat_n(corpus::card("Llanowar Elves")?, 4));
    // Eighteen creatures make the 60 cards.
    for slot in 0..18 {
        let generic = u32::try_from(rng.index(3)).unwrap_or(0);
        let power = i32::try_from(1 + rng.index(4)).unwrap_or(1);
        let toughness = i32::try_from(1 + rng.index(4)).unwrap_or(1);
        let name = format!("Walker Guard {slot} {generic} {power}/{toughness}");
        deck.push(creature(
            &name,
            generic,
            &[colors[rng.index(3)]],
            power,
            toughness,
        ));
    }
    rng.shuffle(&mut deck);
    Ok(deck)
}

/// A normal game of `players` seats whose decks are [`walker_deck`]s, fully determined by `seed`.
pub fn walker_fuzz_setup(players: usize, seed: u64) -> Result<GameSetup, CorpusError> {
    let mut decks = Vec::with_capacity(players);
    for seat in 0u64..players as u64 {
        decks.push(walker_deck(&mut GameRng::substream(seed, 31_000 + seat))?);
    }
    Ok(GameSetup::new(Arc::new(Normal), seed, decks))
}
