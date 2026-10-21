//! A seeded deck of the corpus cards that are cast for an alternative cost or by a permission: Mulldrifter (evoke),
//! Faithless Looting (flashback), Cyclonic Rift and Mizzium Mortars (overload), Omniscience (cast without paying) and
//! Vedalken Orrery (as though flash), with lands of their colours and creatures to aim them at, for fuzz games.

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_core::color::Color;
use mtg_engine::GameSetup;
use mtg_engine::format::Normal;
use mtg_engine::rng::GameRng;

use crate::cards::{creature, land};
use crate::corpus;

/// The corpus cards cast in another way than for their mana cost.
pub const ALTERNATIVE_CARDS: [&str; 6] = [
    "Mulldrifter",
    "Faithless Looting",
    "Cyclonic Rift",
    "Mizzium Mortars",
    "Omniscience",
    "Vedalken Orrery",
];

/// A legal Normal deck of lands, three of each such card and creatures to aim them at, derived entirely from `rng`.
pub fn alternative_deck(rng: &mut GameRng) -> Vec<Arc<CardDefinition>> {
    let colors = [Color::Red, Color::Blue];
    let mut deck: Vec<Arc<CardDefinition>> = (0..28).map(|i| land(colors[i % 2])).collect();
    for card in ALTERNATIVE_CARDS
        .iter()
        .filter_map(|name| corpus::card(name).ok())
    {
        deck.extend(std::iter::repeat_n(card, 3));
    }
    for slot in 0..7 {
        let power = i32::try_from(1 + rng.index(3)).unwrap_or(1);
        let name = format!("Alternative Fodder {slot} {power}/{power}");
        let card = creature(&name, 1, &[colors[rng.index(2)]], power, power);
        deck.extend(std::iter::repeat_n(card, 2));
    }
    rng.shuffle(&mut deck);
    deck.truncate(60);
    deck
}

/// A normal game of `players` seats whose decks are [`alternative_deck`]s, fully determined by `seed`.
pub fn alternative_fuzz_setup(players: usize, seed: u64) -> GameSetup {
    let decks = (0u64..)
        .take(players)
        .map(|seat| alternative_deck(&mut GameRng::substream(seed, 30_000 + seat)))
        .collect();
    GameSetup::new(Arc::new(Normal), seed, decks)
}
