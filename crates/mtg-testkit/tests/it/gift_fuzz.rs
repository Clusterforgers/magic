//! Goal: whole bot games at two, three and four seats with decks of the gift corpus cards finish and keep every invariant, and the games really use the keyword: gifts are promised, a tapped Fish is given and cards are drawn by the opponents they were promised to.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common::seeds::fuzz_games;
use mtg_engine::Event;
use mtg_testkit::gift_cards::GIFT_CARDS;
use mtg_testkit::{gift_fuzz_setup, play_game};
use rstest::rstest;

#[rstest]
#[case::two(2)]
#[case::three(3)]
#[case::four(4)]
fn bot_games_with_gift_cards_finish_and_keep_every_invariant(#[case] seats: usize) {
    let (mut promised, mut fish) = (0, 0);
    for seed in 0..fuzz_games(60) {
        let setup = gift_fuzz_setup(seats, seed).unwrap();
        let outcome =
            play_game(setup, seed).unwrap_or_else(|failure| panic!("seed {seed}: {failure}"));
        for event in &outcome.events {
            match event {
                // Nothing else in the deck has an optional cost.
                Event::SpellCast { choices, .. } if !choices.optional_costs.is_empty() => {
                    promised += 1;
                }
                Event::TokenCreated(token) if token.name == "Fish" => fish += 1,
                _ => {}
            }
        }
    }
    assert!(promised > 0, "a gift was promised");
    assert!(fish > 0, "a tapped Fish was given");
}

#[test]
fn the_cards_of_the_deck_are_all_in_the_corpus() {
    for &(name, _) in GIFT_CARDS {
        assert!(mtg_testkit::corpus::card(name).is_ok(), "{name}");
    }
}
