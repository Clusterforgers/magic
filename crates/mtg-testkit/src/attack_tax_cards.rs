//! A seeded deck of the corpus cards that tax attackers, with lands of their colours, planeswalkers to be attacked and
//! creatures to attack with, for fuzz games.

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_core::color::Color;
use mtg_engine::GameSetup;
use mtg_engine::format::Normal;
use mtg_engine::rng::GameRng;

use crate::cards::{creature, land, walker};
use crate::corpus;

/// The corpus cards that tax attackers.
pub const TAX_CARDS: [&str; 8] = [
    "Propaganda",
    "Ghostly Prison",
    "Windborn Muse",
    "Sphere of Safety",
    "Baird, Steward of Argive",
    "Norn's Annex",
    "Archon of Absolution",
    "Forbidding Spirit",
];

/// A legal Normal deck of mostly white lands, three of each taxing card, planeswalkers and creatures, derived entirely
/// from `rng`.
pub fn tax_deck(rng: &mut GameRng) -> Vec<Arc<CardDefinition>> {
    let colors = [Color::White, Color::Blue, Color::Red, Color::Black];
    let mut deck: Vec<Arc<CardDefinition>> = (0..22)
        .map(|i| land(if i % 4 == 3 { colors[1 + i / 4 % 3] } else { Color::White }))
        .collect();
    for card in TAX_CARDS.iter().filter_map(|name| corpus::card(name).ok()) {
        deck.extend(std::iter::repeat_n(card, 3));
    }
    deck.extend(std::iter::repeat_n(walker("Fuzz Walker", 2), 3));
    // Creatures fill the deck up: they attack, and the taxed ones are what the cards are for.
    for slot in 0..3 {
        let power = i32::try_from(1 + rng.index(3)).unwrap_or(1);
        let name = format!("Tax Fodder {slot} {power}/{power}");
        let card = creature(&name, 1, &[Color::White], power, power);
        deck.extend(std::iter::repeat_n(card, 4));
    }
    rng.shuffle(&mut deck);
    deck.truncate(60);
    deck
}

/// A normal game of `players` seats whose decks are [`tax_deck`]s, fully determined by `seed`.
pub fn tax_fuzz_setup(players: usize, seed: u64) -> GameSetup {
    let decks = (0u64..)
        .take(players)
        .map(|seat| tax_deck(&mut GameRng::substream(seed, 36_000 + seat)))
        .collect();
    GameSetup::new(Arc::new(Normal), seed, decks)
}
