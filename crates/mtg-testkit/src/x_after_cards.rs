//! A seeded deck of the corpus cards that read X after the spell has resolved: Walking Ballista and Hangarback Walker
//! (enter with X counters), Gadwick (an enters trigger that draws X), Disembowel and Forced March (X in what they may
//! destroy) and Wargate (X in the mana value of a search), with lands of their colours and creatures to aim them at,
//! for fuzz games.

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_core::color::Color;
use mtg_engine::GameSetup;
use mtg_engine::format::Normal;
use mtg_engine::rng::GameRng;

use crate::cards::{creature, land};
use crate::corpus;

/// The corpus cards whose X is read after they resolve.
pub const X_AFTER_CARDS: &[&str] = &[
    "Walking Ballista",
    "Hangarback Walker",
    "Gadwick, the Wizened",
    "Disembowel",
    "Forced March",
    "Wargate",
];

/// A legal Normal deck of lands, four of each card and creatures of different mana values to aim them at, derived
/// entirely from `rng`.
pub fn x_after_deck(rng: &mut GameRng) -> Vec<Arc<CardDefinition>> {
    let colors = [
        Color::Blue,
        Color::Black,
        Color::Red,
        Color::Green,
        Color::White,
    ];
    let mut deck: Vec<Arc<CardDefinition>> = (0..28).map(|i| land(colors[i % 5])).collect();
    for card in X_AFTER_CARDS
        .iter()
        .filter_map(|name| corpus::card(name).ok())
    {
        deck.extend(std::iter::repeat_n(card, 4));
    }
    for slot in 0..8 {
        let cost = u32::try_from(1 + rng.index(4)).unwrap_or(1);
        let power = i32::try_from(cost).unwrap_or(1);
        let name = format!("X Fodder {slot} {cost}");
        let card = creature(&name, cost, &[colors[rng.index(3)]], power, power);
        deck.extend(std::iter::repeat_n(card, 2));
    }
    rng.shuffle(&mut deck);
    deck.truncate(60);
    deck
}

/// A normal game of `players` seats whose decks are [`x_after_deck`]s, fully determined by `seed`.
pub fn x_after_fuzz_setup(players: usize, seed: u64) -> GameSetup {
    let decks = (0u64..)
        .take(players)
        .map(|seat| x_after_deck(&mut GameRng::substream(seed, 30_000 + seat)))
        .collect();
    GameSetup::new(Arc::new(Normal), seed, decks)
}
