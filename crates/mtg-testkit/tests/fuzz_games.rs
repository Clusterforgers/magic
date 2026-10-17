//! Goal: whole bot-vs-bot games terminate with exactly one winner or a draw and keep every invariant, for many seeds and table sizes.

use mtg_engine::GameResult;
use mtg_testkit::{fuzz_setup, play_game};
use rstest::rstest;

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
    let mut winners = 0u64;
    let mut draws = 0u64;
    for seed in 0..games() {
        let outcome = play_game(fuzz_setup(players, seed), seed)
            .unwrap_or_else(|failure| panic!("{failure}"));
        match outcome.result {
            GameResult::Winner(_) => winners += 1,
            GameResult::Draw => draws += 1,
        }
    }
    assert_eq!(winners + draws, games());
    assert!(winners > 0, "no game produced a winner out of {}", games());
}
