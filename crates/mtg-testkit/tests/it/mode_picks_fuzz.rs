//! Goal: whole bot games at two, three and four seats with decks of the cards that pick a mode at random or not twice in a turn (Umaro, Lita, Teval's Judgment), the pairs of modes that may both be chosen for a commander (Jeska's Will, Akroma's Will) and the Dragon that puts a card from the hand by the damage dealt (Broodcaller Scourge) finish, keep every invariant of the engine and replay to the state they were played to.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common::seeds::fuzz_games;

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_core::color::Color;
use mtg_engine::GameSetup;
use mtg_engine::format::Normal;
use mtg_engine::rng::GameRng;
use mtg_testkit::cards::{bear, burn, land};
use mtg_testkit::corpus::card;
use mtg_testkit::play_game;
use rstest::rstest;

/// The cards of the deck with their copies.
const CARDS: &[(&str, usize)] = &[
    ("Umaro, Raging Yeti", 3),
    ("Lita, Little Orphan Amphibian", 3),
    ("Teval's Judgment", 3),
    ("Broodcaller Scourge", 3),
    ("Jeska's Will", 3),
    ("Akroma's Will", 3),
];

/// Lands in four colours, the cards above, bears and burn.
fn deck(rng: &mut GameRng) -> Vec<Arc<CardDefinition>> {
    let colors = [Color::Red, Color::Green, Color::White, Color::Black];
    let mut deck: Vec<Arc<CardDefinition>> = (0..35).map(|i| land(colors[i % 4])).collect();
    for &(name, copies) in CARDS {
        deck.extend(std::iter::repeat_n(card(name).unwrap(), copies));
    }
    deck.extend(std::iter::repeat_n(bear(), 4));
    deck.extend(std::iter::repeat_n(burn("Burn", 3), 3));
    rng.shuffle(&mut deck);
    deck
}

fn setup(players: usize, seed: u64) -> GameSetup {
    let decks = (0u64..players as u64)
        .map(|seat| deck(&mut GameRng::substream(seed, 23_000 + seat)))
        .collect();
    GameSetup::new(Arc::new(Normal), seed, decks)
}

#[rstest]
#[case(2)]
#[case(3)]
#[case(4)]
fn seeded_mode_games_finish_and_keep_every_invariant(#[case] players: usize) {
    for seed in 0..fuzz_games(30) {
        play_game(setup(players, seed), seed).unwrap_or_else(|failure| panic!("{failure}"));
    }
}
