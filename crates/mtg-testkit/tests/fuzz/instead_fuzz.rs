//! Goal: whole bot games with decks of the cards whose second sentence replaces the first under a condition, that keep one card of those found, that search for basic lands and/or a land type or that exile a card and ask its kind (Far Wanderings, Primal Growth, Kodama's Reach, Map the Frontier, Circuitous Route, Scavenging Ooze, Lion Sash, Court of Grace) finish and keep every invariant (zones, life, the journal, the replay of the log equal to the live state) for many seeds at tables of two, three and four seats, the games really search, and a seeded game is the same game every time.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common::seeds::fuzz_games;
use mtg_engine::Event;
use mtg_testkit::{instead_fuzz_setup, play_game};
use rstest::rstest;

/// Games per table size; override with `MTG_FUZZ_GAMES`.
fn games() -> u64 {
    fuzz_games(40)
}

#[rstest]
#[case(2)]
#[case(3)]
#[case(4)]
fn seeded_instead_games_finish_and_keep_every_invariant(#[case] players: usize) {
    let mut searches = 0;
    for seed in 0..games() {
        let outcome = play_game(instead_fuzz_setup(players, seed), seed)
            .unwrap_or_else(|failure| panic!("{failure}"));
        searches += outcome
            .events
            .iter()
            .filter(|event| matches!(event, Event::CardsFound(_)))
            .count();
    }
    assert!(
        searches > 0,
        "nothing was searched for in {} games of {players} seats",
        games()
    );
}

#[test]
fn a_seeded_instead_game_is_the_same_game_every_time() {
    for seed in [3, 17, 91] {
        let first = play_game(instead_fuzz_setup(3, seed), seed).unwrap();
        let second = play_game(instead_fuzz_setup(3, seed), seed).unwrap();
        assert_eq!(first.events, second.events);
        assert_eq!(first.final_hash, second.final_hash);
    }
}
