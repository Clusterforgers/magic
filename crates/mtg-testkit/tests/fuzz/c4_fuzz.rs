//! Goal: whole bot games with decks of the cards that gain control for as long as a source stays, take groups, choose cards and move them and count tokens by a group finish and keep every invariant (zones, life, the journal, the replay of the log equal to the live state) for many seeds at tables of two, three and four seats, the games really change control and make tokens, and a seeded game is the same game every time.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common::seeds::fuzz_games;
use mtg_engine::Event;
use mtg_testkit::{c4_fuzz_setup, play_game};
use rstest::rstest;

/// Games per table size; override with `MTG_FUZZ_GAMES`.
fn games() -> u64 {
    fuzz_games(40)
}

#[rstest]
#[case(2)]
#[case(3)]
#[case(4)]
fn seeded_c4_games_finish_and_keep_every_invariant(#[case] players: usize) {
    let (mut effects, mut tokens) = (0, 0);
    for seed in 0..games() {
        let setup = c4_fuzz_setup(players, seed).unwrap();
        let outcome = play_game(setup, seed).unwrap_or_else(|failure| panic!("{failure}"));
        effects += outcome
            .events
            .iter()
            .filter(|event| matches!(event, Event::EffectCreated { .. }))
            .count();
        tokens += outcome
            .events
            .iter()
            .filter(|event| matches!(event, Event::TokenCreated(_)))
            .count();
    }
    assert!(
        effects > 0,
        "no continuous effect began in {} games of {players} seats",
        games()
    );
    assert!(
        tokens > 0,
        "no token was made in {} games of {players} seats",
        games()
    );
}

#[test]
fn a_seeded_c4_game_is_the_same_game_every_time() {
    for seed in [3, 17, 91] {
        let first = play_game(c4_fuzz_setup(3, seed).unwrap(), seed).unwrap();
        let second = play_game(c4_fuzz_setup(3, seed).unwrap(), seed).unwrap();
        assert_eq!(first.events, second.events);
        assert_eq!(first.final_hash, second.final_hash);
    }
}
