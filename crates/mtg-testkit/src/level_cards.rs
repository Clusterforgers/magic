//! Level up, Outlast and Saddle for the fuzz games: the corpus creatures that level up, outlast and saddle, beside
//! plain creatures to pay for them and the lands that cast them.

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_core::color::Color;
use mtg_engine::GameSetup;
use mtg_engine::format::Normal;
use mtg_engine::rng::GameRng;

use crate::cards::{creature, land};
use crate::corpus;

/// The corpus creatures with Level up and their level brackets.
pub const LEVEL_CARDS: &[&str] = &[
    "Student of Warfare",
    "Kazandu Tuskcaller",
    "Coralhelm Commander",
    "Hada Spy Patrol",
    "Caravan Escort",
    "Echo Mage",
];

/// The corpus creatures with Outlast and a static ability for the creatures that hold a +1/+1 counter.
pub const OUTLAST_CARDS: &[&str] = &[
    "Abzan Falconer",
    "Tuskguard Captain",
    "Ainok Bond-Kin",
    "Abzan Battle Priest",
    "Mer-Ek Nightblade",
];

/// The corpus Mount with Saddle.
pub const SADDLE_CARDS: &[&str] = &["Bridled Bighorn"];

/// A legal Normal deck of lands, the corpus creatures that level up, outlast and saddle, and plain creatures of
/// every power, derived entirely from `rng`.
pub fn level_deck(rng: &mut GameRng) -> Vec<Arc<CardDefinition>> {
    let colors = [Color::White, Color::Blue, Color::Green, Color::Black];
    let mut deck: Vec<Arc<CardDefinition>> = (0..24).map(|i| land(colors[i % 4])).collect();
    for name in LEVEL_CARDS.iter().chain(OUTLAST_CARDS).chain(SADDLE_CARDS) {
        if let Ok(card) = corpus::card(name) {
            deck.extend(std::iter::repeat_n(card, 2));
        }
    }
    for slot in 0..12 {
        let power = i32::try_from(1 + rng.index(3)).unwrap_or(1);
        let name = format!("Helper {slot} {power}/{power}");
        deck.push(creature(&name, 1, &[colors[rng.index(4)]], power, power));
    }
    rng.shuffle(&mut deck);
    deck.truncate(60);
    deck
}

/// A normal game of `players` seats whose decks are [`level_deck`]s, fully determined by `seed`.
pub fn level_fuzz_setup(players: usize, seed: u64) -> GameSetup {
    let decks = (0u64..)
        .take(players)
        .map(|seat| level_deck(&mut GameRng::substream(seed, 24_000 + seat)))
        .collect();
    GameSetup::new(Arc::new(Normal), seed, decks)
}
