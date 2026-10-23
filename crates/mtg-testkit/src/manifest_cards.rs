//! A seeded deck of cards that manifest and cloak and the removal and creatures around them, for fuzz games.

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_core::color::Color;
use mtg_engine::GameSetup;
use mtg_engine::format::Normal;
use mtg_engine::rng::GameRng;

use crate::cards::land;
use crate::corpus;

/// The corpus cards the manifest deck is made of besides lands: what manifests and cloaks, what turns a manifested card
/// face up, what kills and exiles it, and the creatures it may be.
pub const MANIFEST_CARDS: &[&str] = &[
    "Reality Shift",
    "Ransom Note",
    "Mastery of the Unseen",
    "Thieving Amalgam",
    "Nantuko Vigilante",
    "Exalted Angel",
    "Serra Angel",
    "Lightning Bolt",
    "Doom Blade",
    "Swords to Plowshares",
    "Sol Ring",
];

/// A legal Normal deck of lands, four of each such card, derived entirely from `rng`.
pub fn manifest_deck(rng: &mut GameRng) -> Vec<Arc<CardDefinition>> {
    let colors = [Color::White, Color::Blue, Color::Black, Color::Red];
    let mut deck: Vec<Arc<CardDefinition>> = (0..24).map(|i| land(colors[i % 4])).collect();
    for card in MANIFEST_CARDS
        .iter()
        .filter_map(|name| corpus::card(name).ok())
    {
        deck.extend(std::iter::repeat_n(card, 4));
    }
    rng.shuffle(&mut deck);
    deck.truncate(60);
    deck
}

/// A normal game of `players` seats whose decks are [`manifest_deck`]s, fully determined by `seed`.
pub fn manifest_fuzz_setup(players: usize, seed: u64) -> GameSetup {
    let decks = (0u64..)
        .take(players)
        .map(|seat| manifest_deck(&mut GameRng::substream(seed, 71_000 + seat)))
        .collect();
    GameSetup::new(Arc::new(Normal), seed, decks)
}
