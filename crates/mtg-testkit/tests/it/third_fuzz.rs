//! Goal: whole bot games with decks of the cards that alone held back two precon cards each in the third round (a creature dealt damage by this creature, another creature returned at the end step, free casts from the hand, bites, the controller of the entering creature, the dealer of damage, shroud after an attach, a return from the graveyard, lands that reveal a card) finish and keep every invariant (zones, life, the journal, the replay of the log equal to the live state) for many seeds at tables of two, three and four seats, the games really use the cards, and a seeded game is the same game every time.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common::seeds::fuzz_games;
use mtg_engine::Event;
use mtg_testkit::{play_game, third_fuzz_setup};
use rstest::rstest;

/// Games per table size; override with `MTG_FUZZ_GAMES`.
fn games() -> u64 {
    fuzz_games(40)
}

#[rstest]
#[case(2)]
#[case(3)]
#[case(4)]
fn seeded_third_round_games_finish_and_keep_every_invariant(#[case] players: usize) {
    let (mut reveals, mut counters) = (0, 0);
    for seed in 0..games() {
        let setup = third_fuzz_setup(players, seed).unwrap();
        let outcome = play_game(setup, seed).unwrap_or_else(|failure| panic!("{failure}"));
        reveals += outcome
            .events
            .iter()
            .filter(|event| matches!(event, Event::Revealed(_)))
            .count();
        counters += outcome
            .events
            .iter()
            .filter(|event| matches!(event, Event::CountersChanged(_)))
            .count();
    }
    assert!(
        reveals > 0,
        "no card was revealed in {} games of {players} seats",
        games()
    );
    assert!(
        counters > 0,
        "no counter was put in {} games of {players} seats",
        games()
    );
}

#[test]
fn a_seeded_third_round_game_is_the_same_game_every_time() {
    for seed in [3, 17, 91] {
        let first = play_game(third_fuzz_setup(3, seed).unwrap(), seed).unwrap();
        let second = play_game(third_fuzz_setup(3, seed).unwrap(), seed).unwrap();
        assert_eq!(first.events, second.events);
        assert_eq!(first.final_hash, second.final_hash);
    }
}
