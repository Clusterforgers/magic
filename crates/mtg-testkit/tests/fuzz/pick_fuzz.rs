//! Goal: whole bot games with decks of cards that take an exact count of X targets or a card at random from a graveyard (Distorting Wake, Entreat the Dead, Back in Town, Surreal Memoir, Charmbreaker Devils, Reap the Past) finish and keep every invariant (zones, life, the journal, the replay of the log equal to the live state) for many seeds at tables of two, three and four seats, the games really pick at random, and a seeded game is the same game every time.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common::seeds::fuzz_games;
use mtg_engine::Event;
use mtg_testkit::{pick_fuzz_setup, play_game};
use rstest::rstest;

/// Games per table size; override with `MTG_FUZZ_GAMES`.
fn games() -> u64 {
    fuzz_games(40)
}

#[rstest]
#[case(2)]
#[case(3)]
#[case(4)]
fn seeded_pick_games_finish_and_keep_every_invariant(#[case] players: usize) {
    let mut picks = 0;
    for seed in 0..games() {
        let setup = pick_fuzz_setup(players, seed).unwrap();
        let outcome = play_game(setup, seed).unwrap_or_else(|failure| panic!("{failure}"));
        picks += outcome
            .events
            .iter()
            .filter(|event| matches!(event, Event::ChosenAtRandom { .. }))
            .count();
    }
    assert!(
        picks > 0,
        "nothing was picked at random in {} games of {players} seats",
        games()
    );
}

#[test]
fn a_seeded_pick_game_is_the_same_game_every_time() {
    for seed in [3, 17, 91] {
        let first = play_game(pick_fuzz_setup(3, seed).unwrap(), seed).unwrap();
        let second = play_game(pick_fuzz_setup(3, seed).unwrap(), seed).unwrap();
        assert_eq!(first.events, second.events);
        assert_eq!(first.final_hash, second.final_hash);
    }
}
