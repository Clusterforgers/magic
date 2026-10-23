//! A seeded deck of the corpus cards whose required additional cost counts X ("pay X life", "exile X cards from your
//! graveyard"), with swamps to cast them, and creatures to fill the graveyard and to sweep, for fuzz games.

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_core::color::Color;
use mtg_engine::GameSetup;
use mtg_engine::format::Normal;
use mtg_engine::rng::GameRng;

use crate::cards::{creature, land};
use crate::corpus;

/// The corpus cards whose required additional cost counts X.
pub const X_COST_CARDS: &[&str] = &["Toxic Deluge", "Skeletal Scrying"];

/// A legal Normal deck of lands, three of each X-cost card and creatures to sweep and to fill the graveyard, derived
/// entirely from `rng`.
pub fn x_cost_deck(rng: &mut GameRng) -> Vec<Arc<CardDefinition>> {
    let mut deck: Vec<Arc<CardDefinition>> = (0..24).map(|_| land(Color::Black)).collect();
    for name in X_COST_CARDS {
        if let Ok(card) = corpus::card(name) {
            deck.extend(std::iter::repeat_n(card, 3));
        }
    }
    for slot in 0..10 {
        let power = i32::try_from(1 + rng.index(3)).unwrap_or(1);
        let name = format!("X Fodder {slot} {power}/{power}");
        let card = creature(&name, 1, &[Color::Black], power, power);
        deck.extend(std::iter::repeat_n(card, 3));
    }
    rng.shuffle(&mut deck);
    deck.truncate(60);
    deck
}

/// A normal game of `players` seats whose decks are [`x_cost_deck`]s, fully determined by `seed`.
pub fn x_cost_fuzz_setup(players: usize, seed: u64) -> GameSetup {
    let decks = (0u64..)
        .take(players)
        .map(|seat| x_cost_deck(&mut GameRng::substream(seed, 31_000 + seat)))
        .collect();
    GameSetup::new(Arc::new(Normal), seed, decks)
}
