//! Goal: bot games with the cards that vote finish at two, three and four seats under every invariant, repeat exactly from their seed, and really vote, each vote an answer of one seat in the log.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common::seeds::fuzz_games;
use mtg_engine::Event;
use mtg_engine::choose::Choice;
use mtg_testkit::{play_game, vote_fuzz_setup};
use rstest::rstest;

#[rstest]
#[case::two(2)]
#[case::three(3)]
#[case::four(4)]
fn bot_games_with_vote_cards_vote_and_keep_every_invariant(#[case] seats: usize) {
    let mut votes = 0usize;
    for seed in 0..fuzz_games(60) {
        let setup = vote_fuzz_setup(seats, seed).unwrap();
        let outcome =
            play_game(setup, seed).unwrap_or_else(|failure| panic!("seed {seed}: {failure}"));
        votes += outcome
            .events
            .iter()
            .filter(|event| {
                matches!(
                    event,
                    Event::ChoiceMade {
                        choice: Choice::Vote(_),
                        ..
                    }
                )
            })
            .count();
    }
    assert!(votes > 0, "a secret vote was cast in the games");
}

#[rstest]
#[case::two(2, 81)]
#[case::four(4, 82)]
fn a_game_with_vote_cards_repeats_exactly_from_its_seed(#[case] seats: usize, #[case] seed: u64) {
    let play = || {
        let setup = vote_fuzz_setup(seats, seed).unwrap();
        play_game(setup, seed).unwrap_or_else(|failure| panic!("seed {seed}: {failure}"))
    };
    let (first, second) = (play(), play());
    assert_eq!(first.events, second.events);
}
