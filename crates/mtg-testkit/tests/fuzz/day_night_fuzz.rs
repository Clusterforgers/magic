//! Goal: whole bot games at two, three and four seats with permanents that start the day as they enter and act when it changes, spells that make it day or night and put creatures onto the battlefield together, a spell that costs less at night and daybound cards keep every invariant, and across the seeds the day begins, changes to night and back, daybound permanents are turned over, and a log replays to the same state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeMap;

use mtg_engine::Event;
use mtg_testkit::{PlayOptions, day_night_fuzz_setup, play_game_with};

use crate::common::seeds::{for_each_seed, fuzz_games};

#[test]
fn bot_games_with_day_and_night_effects_keep_every_invariant() {
    let seen = std::sync::Mutex::new(BTreeMap::<&'static str, u64>::new());
    let games = fuzz_games(15);
    for_each_seed(0..games, |seed| {
        for players in [2, 3, 4] {
            let outcome = play_game_with(
                day_night_fuzz_setup(players, seed),
                seed,
                PlayOptions::default(),
            )
            .unwrap_or_else(|failure| panic!("{failure}"));
            let replayed =
                mtg_engine::replay(&day_night_fuzz_setup(players, seed), &outcome.events)
                    .unwrap_or_else(|error| {
                        panic!("seed {seed}: the log does not replay: {error}")
                    });
            assert_eq!(replayed.state_hash(), outcome.final_hash, "seed {seed}");
            let mut seen = seen.lock().unwrap();
            let mut now = None;
            for event in &outcome.events {
                match event {
                    Event::DayNightChanged { to } => {
                        *seen.entry("day or night began or changed").or_default() += 1;
                        match (now, to) {
                            (None, _) => {}
                            (Some(before), to) if before != *to => {
                                *seen.entry("changed between day and night").or_default() += 1;
                            }
                            (Some(_), _) => panic!("seed {seed}: a change to what it was"),
                        }
                        now = Some(*to);
                    }
                    Event::Transformed { .. } => *seen.entry("turned over").or_default() += 1,
                    Event::AbilityPutOnStack(_) => {
                        *seen.entry("an ability went on the stack").or_default() += 1;
                    }
                    _ => {}
                }
            }
        }
    });
    let seen = seen.into_inner().unwrap();
    for what in [
        "day or night began or changed",
        "changed between day and night",
        "turned over",
        "an ability went on the stack",
    ] {
        assert!(
            seen.contains_key(what),
            "no game had it that {what}: {seen:?}"
        );
    }
}
