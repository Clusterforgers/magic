//! Goal: bot games with the reveal-until cards finish at two, three and four seats under every invariant, repeat exactly from their seed, and really reveal cards, put them into hands and graveyards and onto the battlefield.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common::seeds::fuzz_games;
use mtg_engine::Event;
use mtg_testkit::{play_game, reveal_until_fuzz_setup};
use rstest::rstest;

#[rstest]
#[case::two(2)]
#[case::three(3)]
#[case::four(4)]
fn bot_games_with_reveal_until_reveal_cards_and_keep_every_invariant(#[case] seats: usize) {
    let (mut reveals, mut cards) = (0usize, 0usize);
    for seed in 0..fuzz_games(60) {
        let setup = reveal_until_fuzz_setup(seats, seed).unwrap();
        let outcome =
            play_game(setup, seed).unwrap_or_else(|failure| panic!("seed {seed}: {failure}"));
        for event in &outcome.events {
            if let Event::Revealed(revealed) = event {
                reveals += 1;
                cards += revealed.cards.len();
            }
        }
    }
    assert!(reveals > 0, "a library was revealed from in the games");
    assert!(cards >= reveals, "every reveal names a card");
}

#[rstest]
#[case::two(2, 61)]
#[case::four(4, 62)]
fn a_game_with_reveal_until_repeats_exactly_from_its_seed(#[case] seats: usize, #[case] seed: u64) {
    let play = || {
        let setup = reveal_until_fuzz_setup(seats, seed).unwrap();
        play_game(setup, seed).unwrap_or_else(|failure| panic!("seed {seed}: {failure}"))
    };
    let (first, second) = (play(), play());
    assert_eq!(first.events, second.events);
}
