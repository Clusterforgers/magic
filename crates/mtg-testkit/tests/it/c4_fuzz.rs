//! Goal: whole bot games with decks of the party, chosen-player and prepare cards of (c14a) finish and keep every invariant (zones, life, the journal, no trigger waiting while a player has priority, the replay of the log equal to the live state) for many seeds at tables of two, three and four seats, and the games really use the cards: players are chosen as permanents enter, permanents become prepared by their own triggers and their spell faces are cast, and a seeded game is the same game every time.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeMap;

use mtg_engine::Event;
use mtg_testkit::{c4_fuzz_setup, play_game};
use rstest::rstest;

use crate::common::seeds::fuzz_games;

/// Games per table size; override with `MTG_FUZZ_GAMES`.
fn games() -> u64 {
    fuzz_games(60)
}

#[rstest]
#[case(2)]
#[case(3)]
#[case(4)]
fn seeded_games_finish_and_keep_every_invariant(#[case] players: usize) {
    let mut seen = BTreeMap::<&'static str, u64>::new();
    for seed in 0..games() {
        let setup = c4_fuzz_setup(players, seed).unwrap();
        let outcome = play_game(setup, seed).unwrap_or_else(|failure| panic!("{failure}"));
        for event in &outcome.events {
            match event {
                Event::Entered {
                    chosen: Some(_), ..
                } => *seen.entry("a player chosen").or_default() += 1,
                Event::Prepared { .. } => *seen.entry("prepared").or_default() += 1,
                Event::Unprepared { .. } => *seen.entry("unprepared").or_default() += 1,
                Event::LifeLost { .. } => *seen.entry("life lost").or_default() += 1,
                Event::TokenCreated { .. } => *seen.entry("token").or_default() += 1,
                _ => {}
            }
        }
    }
    let campaign = format!("{} games of {players} seats", games());
    for what in [
        "a player chosen",
        "prepared",
        "unprepared",
        "life lost",
        "token",
    ] {
        assert!(
            seen.contains_key(what),
            "nothing like `{what}` happened in {campaign}: {seen:?}"
        );
    }
}

#[test]
fn a_seeded_game_is_the_same_game_every_time() {
    for seed in [3, 17, 91] {
        let first = play_game(c4_fuzz_setup(3, seed).unwrap(), seed).unwrap();
        let second = play_game(c4_fuzz_setup(3, seed).unwrap(), seed).unwrap();
        assert_eq!(first.events, second.events);
        assert_eq!(first.final_hash, second.final_hash);
    }
}
