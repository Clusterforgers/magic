//! A seeded deck of cards with foretell and the spells and lands around them, for fuzz games.

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_core::color::Color;
use mtg_engine::GameSetup;
use mtg_engine::format::Normal;
use mtg_engine::rng::GameRng;

use crate::cards::land;
use crate::corpus;

/// The corpus cards the foretell deck is made of besides lands: the keyword and what it can be cast against.
pub const FORETELL_CARDS: [&str; 8] = [
    "Behold the Multiverse",
    "Saw It Coming",
    "Demon Bolt",
    "Augury Raven",
    "Lightning Bolt",
    "Grizzly Bears",
    "Counterspell",
    "Opt",
];

/// A legal Normal deck of lands, four of each such card, derived entirely from `rng`.
pub fn foretell_deck(rng: &mut GameRng) -> Vec<Arc<CardDefinition>> {
    let colors = [Color::Blue, Color::Red, Color::Green];
    let mut deck: Vec<Arc<CardDefinition>> = (0..28).map(|i| land(colors[i % 3])).collect();
    for card in FORETELL_CARDS
        .iter()
        .filter_map(|name| corpus::card(name).ok())
    {
        deck.extend(std::iter::repeat_n(card, 4));
    }
    rng.shuffle(&mut deck);
    deck.truncate(60);
    deck
}

/// A normal game of `players` seats whose decks are [`foretell_deck`]s, fully determined by `seed`.
pub fn foretell_fuzz_setup(players: usize, seed: u64) -> GameSetup {
    let decks = (0u64..)
        .take(players)
        .map(|seat| foretell_deck(&mut GameRng::substream(seed, 60_000 + seat)))
        .collect();
    GameSetup::new(Arc::new(Normal), seed, decks)
}
