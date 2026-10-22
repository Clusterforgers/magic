//! A seeded deck of the corpus cards with prowl and the Rogues that open it, with lands of their colours and creatures to
//! block and fill the graveyards, for fuzz games.

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_core::color::Color;
use mtg_engine::GameSetup;
use mtg_engine::format::Normal;
use mtg_engine::rng::GameRng;

use crate::cards::{creature, land};
use crate::corpus;

/// The corpus cards with prowl.
pub const PROWL_CARDS: [&str; 2] = ["Latchkey Faerie", "Morsel Theft"];

/// A legal Normal deck of lands, four of each such card and creatures, derived entirely from `rng`.
pub fn prowl_deck(rng: &mut GameRng) -> Vec<Arc<CardDefinition>> {
    let colors = [Color::Blue, Color::Black, Color::Green, Color::Red];
    let mut deck: Vec<Arc<CardDefinition>> = (0..24).map(|i| land(colors[i % 2])).collect();
    for card in PROWL_CARDS
        .iter()
        .filter_map(|name| corpus::card(name).ok())
    {
        deck.extend(std::iter::repeat_n(card, 4));
    }
    // Rogues attack and open prowl; the other creatures fill the deck up, block and die.
    let mut slot = 0;
    while deck.len() < 60 {
        let power = i32::try_from(1 + rng.index(3)).unwrap_or(1);
        let name = format!("Prowl Fodder {slot} {power}/{power}");
        let card = creature(&name, 1, &[colors[rng.index(2)]], power, power);
        let card = if slot % 2 == 0 {
            Arc::new(CardDefinition::clone(&card).with_subtype("Rogue"))
        } else {
            card
        };
        deck.extend(std::iter::repeat_n(card, 3));
        slot += 1;
    }
    rng.shuffle(&mut deck);
    deck.truncate(60);
    deck
}

/// A normal game of `players` seats whose decks are [`prowl_deck`]s, fully determined by `seed`.
pub fn prowl_fuzz_setup(players: usize, seed: u64) -> GameSetup {
    let decks = (0u64..)
        .take(players)
        .map(|seat| prowl_deck(&mut GameRng::substream(seed, 34_000 + seat)))
        .collect();
    GameSetup::new(Arc::new(Normal), seed, decks)
}
