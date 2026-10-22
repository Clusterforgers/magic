//! A seeded deck of cards with morph and megamorph and the removal and targets around them, for fuzz games.

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_core::color::Color;
use mtg_engine::GameSetup;
use mtg_engine::format::Normal;
use mtg_engine::rng::GameRng;

use crate::cards::land;
use crate::corpus;

/// The corpus cards the morph deck is made of besides lands: the keywords, and what kills, exiles and is destroyed by them.
pub const MORPH_CARDS: &[&str] = &[
    "Nantuko Vigilante",
    "Ainok Survivalist",
    "Hidden Dragonslayer",
    "Master of Pearls",
    "Exalted Angel",
    "Riptide Survivor",
    "Stratus Dancer",
    "Thelonite Hermit",
    "Krosan Colossus",
    "Zoetic Cavern",
    "Lightning Bolt",
    "Doom Blade",
    "Swords to Plowshares",
    "Pacifism",
    "Sol Ring",
];

/// A legal Normal deck of lands, four of each such card, derived entirely from `rng`.
pub fn morph_deck(rng: &mut GameRng) -> Vec<Arc<CardDefinition>> {
    let colors = [Color::Green, Color::White, Color::Blue, Color::Red];
    let mut deck: Vec<Arc<CardDefinition>> = (0..24).map(|i| land(colors[i % 4])).collect();
    for card in MORPH_CARDS
        .iter()
        .filter_map(|name| corpus::card(name).ok())
    {
        deck.extend(std::iter::repeat_n(card, 4));
    }
    rng.shuffle(&mut deck);
    deck.truncate(60);
    deck
}

/// A normal game of `players` seats whose decks are [`morph_deck`]s, fully determined by `seed`.
pub fn morph_fuzz_setup(players: usize, seed: u64) -> GameSetup {
    let decks = (0u64..)
        .take(players)
        .map(|seat| morph_deck(&mut GameRng::substream(seed, 70_000 + seat)))
        .collect();
    GameSetup::new(Arc::new(Normal), seed, decks)
}
