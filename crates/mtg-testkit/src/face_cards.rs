//! A seeded deck of cards with several faces and the spells and lands around them, for fuzz games.

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_core::color::Color;
use mtg_engine::GameSetup;
use mtg_engine::format::Normal;
use mtg_engine::rng::GameRng;

use crate::cards::land;
use crate::corpus;

/// The corpus cards the face deck is made of besides lands: split cards (one with fuse) and the cards they target, an
/// Adventure, two modal double-faced cards and transforming ones (an upkeep trigger, an activated ability, day and night).
pub const FACE_CARDS: [&str; 13] = [
    "Fire // Ice",
    "Bonecrusher Giant // Stomp",
    "Hengegate Pathway // Mistgate Pathway",
    "Selfless Glyphweaver // Deadly Vanity",
    "Lightning Bolt",
    "Grizzly Bears",
    "Counterspell",
    "Delver of Secrets // Insectile Aberration",
    "Ludevic's Test Subject // Ludevic's Abomination",
    "Tavern Ruffian // Tavern Smasher",
    "Wear // Tear",
    "Sol Ring",
    "Glorious Anthem",
];

/// A legal Normal deck of lands and three of each such card, derived entirely from `rng`.
pub fn face_deck(rng: &mut GameRng) -> Vec<Arc<CardDefinition>> {
    let colors = [Color::Red, Color::Blue, Color::White, Color::Black];
    let mut deck: Vec<Arc<CardDefinition>> = (0..24).map(|i| land(colors[i % 4])).collect();
    for card in FACE_CARDS.iter().filter_map(|name| corpus::card(name).ok()) {
        deck.extend(std::iter::repeat_n(card, 3));
    }
    rng.shuffle(&mut deck);
    deck.truncate(60);
    deck
}

/// A normal game of `players` seats whose decks are [`face_deck`]s, fully determined by `seed`.
pub fn face_fuzz_setup(players: usize, seed: u64) -> GameSetup {
    let decks = (0u64..)
        .take(players)
        .map(|seat| face_deck(&mut GameRng::substream(seed, 70_000 + seat)))
        .collect();
    GameSetup::new(Arc::new(Normal), seed, decks)
}
