//! Goal: whole bot-vs-bot games terminate with exactly one winner or a draw and keep every invariant, for many seeds and table sizes.

use std::sync::atomic::{AtomicU64, Ordering};

use mtg_engine::GameResult;
use mtg_testkit::{PlayOptions, fuzz_setup, play_game, play_game_with};
use rstest::rstest;

use crate::common::seeds::for_each_seed;

/// Games per table size; override with `MTG_FUZZ_GAMES` to run longer or shorter campaigns.
fn games() -> u64 {
    std::env::var("MTG_FUZZ_GAMES")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(500)
}

#[rstest]
#[case(2)]
#[case(3)]
#[case(4)]
#[case(6)]
fn seeded_bot_games_finish_with_a_winner_or_a_draw(#[case] players: usize) {
    let (winners, draws) = (AtomicU64::new(0), AtomicU64::new(0));
    for_each_seed(0..games(), |seed| {
        let outcome = play_game(fuzz_setup(players, seed), seed)
            .unwrap_or_else(|failure| panic!("{failure}"));
        match outcome.result {
            GameResult::Winner(_) => winners.fetch_add(1, Ordering::Relaxed),
            GameResult::Draw => draws.fetch_add(1, Ordering::Relaxed),
        };
    });
    let (winners, draws) = (winners.into_inner(), draws.into_inner());
    assert_eq!(winners + draws, games());
    assert!(winners > 0, "no game produced a winner out of {}", games());
}

#[rstest]
#[case(2)]
#[case(4)]
fn seeded_bot_games_that_let_the_engine_finish_payments_finish_too(#[case] players: usize) {
    let options = PlayOptions {
        partial_payments: true,
        ..PlayOptions::default()
    };
    for_each_seed(0..games() / 5, |seed| {
        play_game_with(fuzz_setup(players, seed), seed, options)
            .unwrap_or_else(|failure| panic!("{failure}"));
    });
}
