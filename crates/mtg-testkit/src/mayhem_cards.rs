//! A seeded deck of the corpus cards with mayhem and the cards that make their owners discard, with lands of their colours,
//! for fuzz games.

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_core::color::Color;
use mtg_engine::GameSetup;
use mtg_engine::format::Normal;
use mtg_engine::rng::GameRng;

use crate::cards::land;
use crate::corpus;

/// The corpus cards with mayhem.
pub const MAYHEM_CARDS: [&str; 8] = [
    "Sadistic Slash",
    "Swarm, Being of Bees",
    "Spider-Islanders",
    "The Squadron Sinister",
    "Green Goblin, Back for More",
    "Raging Goblinoids",
    "Abomination, World Ravager",
    "Electro's Bolt",
];

/// The corpus cards that discard cards, as a cost or an effect, so that the cards above reach a graveyard in the turn.
pub const DISCARD_CARDS: [&str; 6] = [
    "Faithless Looting",
    "Rummaging Goblin",
    "Thrill of Possibility",
    "Looter il-Kor",
    "Smuggler's Copter",
    "Laughing Mad",
];

/// A legal Normal deck of lands, three of each such card and the discarding cards, derived entirely from `rng`.
pub fn mayhem_deck(rng: &mut GameRng) -> Vec<Arc<CardDefinition>> {
    let colors = [Color::Black, Color::Red, Color::Blue];
    let mut deck: Vec<Arc<CardDefinition>> = (0..24).map(|i| land(colors[i % 3])).collect();
    for card in MAYHEM_CARDS
        .iter()
        .chain(&DISCARD_CARDS)
        .filter_map(|name| corpus::card(name).ok())
    {
        deck.extend(std::iter::repeat_n(card, 3));
    }
    rng.shuffle(&mut deck);
    deck.truncate(60);
    deck
}

/// A normal game of `players` seats whose decks are [`mayhem_deck`]s, fully determined by `seed`.
pub fn mayhem_fuzz_setup(players: usize, seed: u64) -> GameSetup {
    let decks = (0u64..)
        .take(players)
        .map(|seat| mayhem_deck(&mut GameRng::substream(seed, 38_000 + seat)))
        .collect();
    GameSetup::new(Arc::new(Normal), seed, decks)
}
