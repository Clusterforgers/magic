//! Triggers on the events about players and permanents (life, damage, draws, discards, cycling, taps, untaps and mana
//! taps), and the decks and games built from the corpus cards that wait for them.

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_core::color::Color;
use mtg_engine::GameSetup;
use mtg_engine::format::Normal;
use mtg_engine::rng::GameRng;

use crate::cards::{bear, burn, land};
use crate::corpus::{self, CorpusError};
use crate::part_cards::cycler;

/// Lands in an events deck: black twice and three other colours in turn.
const LANDS: usize = 25;

/// The corpus cards of the event triggers, with their copies in a deck.
pub const EVENT_CARDS: [(&str, usize); 8] = [
    ("Ajani's Pridemate", 3),
    ("Thieving Magpie", 3),
    ("Underworld Dreams", 3),
    ("Megrim", 2),
    ("Manabarbs", 2),
    ("Mesmeric Orb", 2),
    ("Wanderbrine Preacher", 3),
    ("Renewed Faith", 3),
];

/// The deck of a seat: lands in four colours (black most), every event card, cyclers and discarders for the cycling and discard
/// events, and creatures and burn to attack, tap and damage with.
pub fn events_deck(rng: &mut GameRng) -> Result<Vec<Arc<CardDefinition>>, CorpusError> {
    let colors = [
        Color::Black,
        Color::Blue,
        Color::White,
        Color::Black,
        Color::Red,
    ];
    let mut deck: Vec<Arc<CardDefinition>> = (0..LANDS).map(|i| land(colors[i % 5])).collect();
    for (name, copies) in EVENT_CARDS {
        deck.extend(std::iter::repeat_n(corpus::card(name)?, copies));
    }
    for (name, copies) in [
        ("Rummaging Goblin", 2),
        ("Soul Shepherd", 2),
        ("Kor Firewalker", 2),
        ("Lurching Rotbeast", 2),
    ] {
        deck.extend(std::iter::repeat_n(corpus::card(name)?, copies));
    }
    deck.extend(std::iter::repeat_n(cycler("Cycler"), 3));
    deck.extend(std::iter::repeat_n(bear(), 4));
    deck.extend(std::iter::repeat_n(burn("Burn", 3), 3));
    rng.shuffle(&mut deck);
    Ok(deck)
}

/// A game of `players` seats with event decks.
pub fn events_fuzz_setup(players: usize, seed: u64) -> Result<GameSetup, CorpusError> {
    let mut decks = Vec::with_capacity(players);
    for seat in 0u64..players as u64 {
        decks.push(events_deck(&mut GameRng::substream(seed, 19_000 + seat))?);
    }
    Ok(GameSetup::new(Arc::new(Normal), seed, decks))
}
