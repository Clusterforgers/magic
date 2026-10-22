//! A seeded deck of keyword actions and designations (surveil, the monarch and the other actions of the rules'
//! section 701), with lands of their colours and creatures to attack with, for fuzz games.

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_core::color::Color;
use mtg_engine::GameSetup;
use mtg_engine::format::Normal;
use mtg_engine::rng::GameRng;

use crate::cards::{creature, land};
use crate::corpus;

/// The corpus cards the keyword-action deck is made of besides lands and creatures.
pub const ACTION_CARDS: [&str; 5] = [
    "Consider",
    "Curate",
    "Notion Rain",
    "Sinister Sabotage",
    "Doom Whisperer",
];

/// A legal Normal deck in blue and black of the cards that act, three of each, with creatures to fill it, derived
/// entirely from `rng`.
pub fn actions_deck(rng: &mut GameRng) -> Vec<Arc<CardDefinition>> {
    let colors = [Color::Blue, Color::Black];
    let mut deck: Vec<Arc<CardDefinition>> = (0..25).map(|i| land(colors[i % 2])).collect();
    for card in ACTION_CARDS
        .iter()
        .filter_map(|name| corpus::card(name).ok())
    {
        deck.extend(std::iter::repeat_n(card, 3));
    }
    // Creatures make the 60 cards.
    for slot in 0..10 {
        let generic = u32::try_from(rng.index(3)).unwrap_or(0);
        let power = i32::try_from(1 + rng.index(4)).unwrap_or(1);
        let toughness = i32::try_from(1 + rng.index(4)).unwrap_or(1);
        let name = format!("Action Fodder {slot} {generic} {power}/{toughness}");
        let card = creature(&name, generic, &[colors[rng.index(2)]], power, toughness);
        deck.extend(std::iter::repeat_n(card, 2));
    }
    rng.shuffle(&mut deck);
    deck.truncate(60);
    deck
}

/// A normal game of `players` seats whose decks are [`actions_deck`]s, fully determined by `seed`.
pub fn actions_fuzz_setup(players: usize, seed: u64) -> GameSetup {
    let decks = (0u64..)
        .take(players)
        .map(|seat| actions_deck(&mut GameRng::substream(seed, 300_000 + seat)))
        .collect();
    GameSetup::new(Arc::new(Normal), seed, decks)
}
