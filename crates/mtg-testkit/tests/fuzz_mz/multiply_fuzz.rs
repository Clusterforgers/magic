//! Goal: whole bot games at two, three and four seats with the doubling cards (Kalonian Hydra, Primordial Hydra, Big Mother Mouser, Vorel of the Hull Clade) and Genesis Storm keep every invariant and replay from the log to the final state, and across the seeds counters are multiplied (a permanent that already holds counters gets as many again), a reveal until a card matches resolves and puts the rest on the bottom in a random order, and a seeded game is the same game every time.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::{BTreeMap, HashMap};

use mtg_engine::choose::Target;
use mtg_engine::{Event, replay};
use mtg_testkit::{PlayOptions, multiply_fuzz_setup, play_game_with};
use rstest::rstest;

use crate::common::seeds::{for_each_seed, fuzz_games};

#[rstest]
#[case(2)]
#[case(3)]
#[case(4)]
fn bot_games_with_the_doubling_cards_keep_every_invariant_and_multiply_counters(
    #[case] players: usize,
) {
    let seen = std::sync::Mutex::new(BTreeMap::<&'static str, u64>::new());
    for_each_seed(0..fuzz_games(20), |seed| {
        let setup = multiply_fuzz_setup(players, seed);
        let outcome = play_game_with(setup.clone(), seed, PlayOptions::default())
            .unwrap_or_else(|failure| panic!("{failure}"));
        let end = replay(&setup, &outcome.events).unwrap();
        assert_eq!(end.state_hash(), outcome.final_hash, "seed {seed}");
        let mut held: HashMap<_, i64> = HashMap::new();
        let mut seen = seen.lock().unwrap();
        for event in &outcome.events {
            match event {
                // The deck has no other source of counters after the ones a permanent enters with, so a change onto a
                // permanent that holds some already is a multiplication, and it adds exactly what was held.
                Event::CountersChanged(changed) => {
                    if let Target::Object(object) = changed.on {
                        let before = held.entry(object).or_default();
                        if changed.delta > 0 && *before > 0 {
                            assert_eq!(i64::from(changed.delta), *before, "seed {seed}");
                            *seen.entry("counters multiplied").or_default() += 1;
                        }
                        *before += i64::from(changed.delta);
                    }
                }
                Event::Revealed(_) => *seen.entry("revealed").or_default() += 1,
                Event::LibraryBottomed { random: true, .. } => {
                    *seen.entry("bottomed at random").or_default() += 1;
                }
                _ => {}
            }
        }
    });
    let seen = seen.into_inner().unwrap();
    for what in ["counters multiplied", "revealed", "bottomed at random"] {
        assert!(
            seen.get(what).copied().unwrap_or(0) > 0,
            "never seen at {players} seats: {what}: {seen:?}"
        );
    }
}

#[test]
fn a_seeded_game_with_the_doubling_cards_is_the_same_game_every_time() {
    for seed in [3, 17, 91] {
        let first = play_game_with(multiply_fuzz_setup(3, seed), seed, PlayOptions::default());
        let second = play_game_with(multiply_fuzz_setup(3, seed), seed, PlayOptions::default());
        let (first, second) = (first.unwrap(), second.unwrap());
        assert_eq!(first.events, second.events);
        assert_eq!(first.final_hash, second.final_hash);
    }
}
