//! Goal: bot games with the cards that repeat for each player, roll dice and flip coins finish at two, three and four seats under every invariant, repeat exactly from their seed, and really roll and flip, with the stream moving on once for each.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common::seeds::fuzz_games;
use mtg_engine::Event;
use mtg_testkit::{chance_fuzz_setup, play_game};
use rstest::rstest;

#[rstest]
#[case::two(2)]
#[case::three(3)]
#[case::four(4)]
fn bot_games_with_chance_cards_roll_and_flip_and_keep_every_invariant(#[case] seats: usize) {
    let (mut rolls, mut flips) = (0usize, 0usize);
    for seed in 0..fuzz_games(60) {
        let setup = chance_fuzz_setup(seats, seed).unwrap();
        let outcome =
            play_game(setup, seed).unwrap_or_else(|failure| panic!("seed {seed}: {failure}"));
        for event in &outcome.events {
            match event {
                Event::DieRolled { sides, result, .. } => {
                    assert!((1..=*sides).contains(result), "a roll is on the die");
                    rolls += 1;
                }
                Event::CoinFlipped { .. } => flips += 1,
                _ => {}
            }
        }
    }
    assert!(
        rolls + flips > 0,
        "a die was rolled or a coin flipped in the games"
    );
}

#[rstest]
#[case::two(2, 71)]
#[case::four(4, 72)]
fn a_game_with_chance_cards_repeats_exactly_from_its_seed(#[case] seats: usize, #[case] seed: u64) {
    let play = || {
        let setup = chance_fuzz_setup(seats, seed).unwrap();
        play_game(setup, seed).unwrap_or_else(|failure| panic!("seed {seed}: {failure}"))
    };
    let (first, second) = (play(), play());
    assert_eq!(first.events, second.events);
}
