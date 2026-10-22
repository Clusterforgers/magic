//! Amounts counted by "for each" (cards drawn, life, mana, tokens, damage bonuses and shrinking spells), and the decks
//! and games built from the corpus cards that count.

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_core::color::Color;
use mtg_engine::GameSetup;
use mtg_engine::format::Normal;
use mtg_engine::rng::GameRng;

use crate::cards::{bear, land};
use crate::corpus::{self, CorpusError};

/// Lands in a counting deck: the five colours in turn.
const LANDS: usize = 22;

/// The corpus cards that count, with their copies in a deck.
pub const COUNT_CARDS: &[(&str, usize)] = &[
    ("Collective Unconscious", 2),
    ("Keep Watch", 2),
    ("Brightstone Ritual", 2),
    ("Elven Ambush", 2),
    ("Defile", 2),
    ("Borderland Behemoth", 3),
    ("Ethereal Armor", 2),
    ("Empyrial Plate", 2),
    ("Gerrard's Wisdom", 2),
    ("Pygmy Kavu", 3),
    ("Circle of Dreams Druid", 3),
    ("Songs of the Damned", 2),
];

/// The deck of a seat: lands in five colours, every card that counts, and the Elves, Goblins and bears they count and
/// attack with.
pub fn count_deck(rng: &mut GameRng) -> Result<Vec<Arc<CardDefinition>>, CorpusError> {
    let colors = [
        Color::Green,
        Color::Red,
        Color::Black,
        Color::White,
        Color::Blue,
    ];
    let mut deck: Vec<Arc<CardDefinition>> = (0..LANDS).map(|i| land(colors[i % 5])).collect();
    for &(name, copies) in COUNT_CARDS {
        deck.extend(std::iter::repeat_n(corpus::card(name)?, copies));
    }
    for (name, copies) in [("Llanowar Elves", 4), ("Rummaging Goblin", 3)] {
        deck.extend(std::iter::repeat_n(corpus::card(name)?, copies));
    }
    deck.extend(std::iter::repeat_n(bear(), 4));
    rng.shuffle(&mut deck);
    Ok(deck)
}

/// A game of `players` seats with counting decks.
pub fn count_fuzz_setup(players: usize, seed: u64) -> Result<GameSetup, CorpusError> {
    let mut decks = Vec::with_capacity(players);
    for seat in 0u64..players as u64 {
        decks.push(count_deck(&mut GameRng::substream(seed, 47_000 + seat))?);
    }
    Ok(GameSetup::new(Arc::new(Normal), seed, decks))
}
