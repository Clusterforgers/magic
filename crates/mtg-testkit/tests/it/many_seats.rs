//! Goal: a game has no cap on its seats: seeded bot games of 8, 16 and 24 Normal seats and of 4 and 8 Commander seats finish with every invariant, and a game takes a number of answers that grows with the square of its table (every seat passes at every step of every turn), not with its cube.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common::seeds::{for_each_seed, fuzz_games};

use std::time::Instant;

use mtg_testkit::{commander_fuzz_setup, fuzz_setup, play_game};
use rstest::rstest;

/// Games per table size; override with `MTG_FUZZ_GAMES`.
fn games() -> u64 {
    fuzz_games(2)
}

/// Answers per squared seat a bot game may take: the games of 8 to 24 seats take about 180.
const ANSWERS_PER_SQUARE: usize = 400;

/// The same for Commander, whose 100-card decks and 40 life make longer games: about 420.
const COMMANDER_ANSWERS_PER_SQUARE: usize = 1_000;

#[rstest]
#[case(8)]
#[case(16)]
#[case(24)]
fn seeded_normal_games_of_many_seats_finish_and_keep_every_invariant(#[case] seats: usize) {
    let started = Instant::now();
    for_each_seed(0..games(), |seed| {
        let outcome =
            play_game(fuzz_setup(seats, seed), seed).unwrap_or_else(|failure| panic!("{failure}"));
        eprintln!("{seats} seats, seed {seed}: {} answers", outcome.answers);
        assert!(
            outcome.answers < ANSWERS_PER_SQUARE * seats * seats,
            "{seats} seats took {} answers",
            outcome.answers
        );
    });
    eprintln!("{seats} seats: {:?}", started.elapsed());
}

#[rstest]
#[case(4)]
#[case(8)]
fn seeded_commander_games_of_many_seats_finish_and_keep_every_invariant(#[case] seats: usize) {
    let started = Instant::now();
    for_each_seed(0..games(), |seed| {
        let setup = commander_fuzz_setup(seats, seed).unwrap();
        let outcome = play_game(setup, seed).unwrap_or_else(|failure| panic!("{failure}"));
        eprintln!("{seats} seats, seed {seed}: {} answers", outcome.answers);
        assert!(outcome.answers < COMMANDER_ANSWERS_PER_SQUARE * seats * seats);
    });
    eprintln!("{seats} commander seats: {:?}", started.elapsed());
}
