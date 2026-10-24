//! Goal: whole bot games with decks of the cards that give keyword counters (Unbreakable Bond, Assemble the Entmoot), search for as many lands as a count names (Harvest Season, Traverse the Outlands) and proliferate (Contentious Plan) finish at two, three and four seats under every invariant, a keyword counter is put in some of them, the same seed plays the same game, and the log replays.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common::seeds::fuzz_games;
use mtg_engine::Event;
use mtg_testkit::{counter_fuzz_setup, play_game};
use rstest::rstest;

/// Games per table size; override with `MTG_FUZZ_GAMES`.
fn games() -> u64 {
    fuzz_games(40)
}

#[rstest]
#[case(2)]
#[case(3)]
#[case(4)]
fn seeded_counter_games_finish_and_keep_every_invariant(#[case] players: usize) {
    let mut keyword_counters = 0;
    for seed in 0..games() {
        let setup = counter_fuzz_setup(players, seed).unwrap();
        let outcome = play_game(setup, seed).unwrap_or_else(|failure| panic!("{failure}"));
        keyword_counters += outcome
            .events
            .iter()
            .filter(|event| {
                matches!(event, Event::CountersChanged(changed)
                    if changed.delta > 0 && changed.counter.keyword().is_some())
            })
            .count();
    }
    assert!(
        keyword_counters > 0,
        "no keyword counter was put in {} games of {players} seats",
        games()
    );
}

#[test]
fn a_seeded_counter_game_is_the_same_game_every_time() {
    for seed in [3, 17, 91] {
        let first = play_game(counter_fuzz_setup(3, seed).unwrap(), seed).unwrap();
        let second = play_game(counter_fuzz_setup(3, seed).unwrap(), seed).unwrap();
        assert_eq!(first.events, second.events);
        assert_eq!(first.final_hash, second.final_hash);
    }
}
