//! A seeded deck of the corpus cards whose second sentence replaces the first under a condition (Far Wanderings,
//! Primal Growth, Court of Grace), that keep one card of those found (Kodama's Reach), that search for basic lands
//! and/or a land type (Map the Frontier, Circuitous Route) or that exile a card and ask its kind (Scavenging Ooze, Lion
//! Sash), with lands of their colours and creatures to fill the graveyards, for fuzz games.

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_core::color::Color;
use mtg_engine::GameSetup;
use mtg_engine::format::Normal;
use mtg_engine::rng::GameRng;

use crate::cards::{creature, land};
use crate::corpus;

/// The corpus cards with a replacement under a condition, a split put or an exile that asks the kind of the card.
pub const INSTEAD_CARDS: &[(&str, usize)] = &[
    ("Far Wanderings", 3),
    ("Primal Growth", 3),
    ("Kodama's Reach", 3),
    ("Map the Frontier", 2),
    ("Circuitous Route", 2),
    ("Scavenging Ooze", 3),
    ("Lion Sash", 2),
    ("Court of Grace", 2),
    ("Zimone, Quandrix Prodigy", 1),
];

/// A legal Normal deck of lands, the cards of [`INSTEAD_CARDS`] and creatures to fill the graveyards, derived entirely
/// from `rng`.
pub fn instead_deck(rng: &mut GameRng) -> Vec<Arc<CardDefinition>> {
    let colors = [Color::Green, Color::White, Color::Blue];
    let mut deck: Vec<Arc<CardDefinition>> = (0..24).map(|i| land(colors[i % 3])).collect();
    for (name, copies) in INSTEAD_CARDS {
        if let Ok(card) = corpus::card(name) {
            deck.extend(std::iter::repeat_n(card, *copies));
        }
    }
    for slot in 0..10 {
        let power = i32::try_from(1 + rng.index(3)).unwrap_or(1);
        let name = format!("Instead Fodder {slot} {power}/{power}");
        let card = creature(&name, 1, &[colors[rng.index(3)]], power, power);
        deck.extend(std::iter::repeat_n(card, 2));
    }
    rng.shuffle(&mut deck);
    deck.truncate(60);
    deck
}

/// A normal game of `players` seats whose decks are [`instead_deck`]s, fully determined by `seed`.
pub fn instead_fuzz_setup(players: usize, seed: u64) -> GameSetup {
    let decks = (0u64..)
        .take(players)
        .map(|seat| instead_deck(&mut GameRng::substream(seed, 52_000 + seat)))
        .collect();
    GameSetup::new(Arc::new(Normal), seed, decks)
}
