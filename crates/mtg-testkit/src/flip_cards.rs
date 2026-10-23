//! A seeded deck of flip cards and the Spirit creatures that feed their ki counters, for fuzz games.

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::vanilla::creature;
use mtg_core::color::Color;
use mtg_engine::GameSetup;
use mtg_engine::format::Normal;
use mtg_engine::rng::GameRng;

use crate::cards::land;
use crate::corpus;

/// The corpus flip cards of the deck: two that count a hand or a board (an activated ability), and two ki-counter
/// cards (a spell trigger and an end step trigger).
pub const FLIP_CARDS: &[&str] = &[
    "Jushi Apprentice // Tomoya the Revealer",
    "Orochi Eggwatcher // Shidako, Broodmistress",
    "Budoka Pupil // Ichiga, Who Topples Oaks",
    "Hired Muscle // Scarmaker",
];

/// A 1/1 Spirit creature card named `name`: casting it puts a ki counter on a ki-counter card.
pub fn spirit_named(name: &str) -> Arc<CardDefinition> {
    Arc::new(
        (*creature(name, 0, &[Color::Blue], 1, 1))
            .clone()
            .with_subtype("Spirit"),
    )
}

/// The 1/1 Spirit creature card "Wisp".
pub fn spirit() -> Arc<CardDefinition> {
    spirit_named("Wisp")
}

/// A legal Normal deck of lands, four of each flip card and a dozen Spirit creatures, derived entirely from `rng`.
pub fn flip_deck(rng: &mut GameRng) -> Vec<Arc<CardDefinition>> {
    let colors = [Color::Blue, Color::Green, Color::Black, Color::White];
    let mut deck: Vec<Arc<CardDefinition>> = (0..32).map(|i| land(colors[i % 4])).collect();
    for card in FLIP_CARDS.iter().filter_map(|name| corpus::card(name).ok()) {
        deck.extend(std::iter::repeat_n(card, 4));
    }
    for name in ["Wisp", "Wraith", "Shade", "Ghost"] {
        deck.extend(std::iter::repeat_n(spirit_named(name), 3));
    }
    rng.shuffle(&mut deck);
    deck.truncate(60);
    deck
}

/// A normal game of `players` seats whose decks are [`flip_deck`]s, fully determined by `seed`.
pub fn flip_fuzz_setup(players: usize, seed: u64) -> GameSetup {
    let decks = (0u64..)
        .take(players)
        .map(|seat| flip_deck(&mut GameRng::substream(seed, 80_000 + seat)))
        .collect();
    GameSetup::new(Arc::new(Normal), seed, decks)
}
