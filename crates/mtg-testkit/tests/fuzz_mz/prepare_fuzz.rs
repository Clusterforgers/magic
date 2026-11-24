//! Goal: whole bot games at two, three and four seats with prepare cards (one that enters prepared, one prepared in its controller's upkeep, one that attacks to be prepared) keep every invariant, and across the seeds permanents become prepared, copies of spell faces are cast from the battlefield and unprepare them, and a log replays to the same state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeMap;

use mtg_engine::Event;
use mtg_testkit::{PlayOptions, play_game_with, prepare_fuzz_setup};

use crate::common::seeds::{for_each_seed, fuzz_games};

#[test]
fn bot_games_with_prepare_cards_keep_every_invariant() {
    let seen = std::sync::Mutex::new(BTreeMap::<&'static str, u64>::new());
    let games = fuzz_games(15);
    for_each_seed(0..games, |seed| {
        for players in [2, 3, 4] {
            let outcome = play_game_with(
                prepare_fuzz_setup(players, seed),
                seed,
                PlayOptions::default(),
            )
            .unwrap_or_else(|failure| panic!("{failure}"));
            let replayed = mtg_engine::replay(&prepare_fuzz_setup(players, seed), &outcome.events)
                .unwrap_or_else(|error| panic!("seed {seed}: the log does not replay: {error}"));
            assert_eq!(replayed.state_hash(), outcome.final_hash, "seed {seed}");
            let mut seen = seen.lock().unwrap();
            for event in &outcome.events {
                match event {
                    Event::Prepared { .. } => *seen.entry("prepared").or_default() += 1,
                    Event::Unprepared { .. } => *seen.entry("unprepared").or_default() += 1,
                    Event::CardCopied(_) => *seen.entry("copied").or_default() += 1,
                    Event::CopyCeased { .. } => *seen.entry("copy ceased").or_default() += 1,
                    _ => {}
                }
            }
        }
    });
    let seen = seen.into_inner().unwrap();
    for what in ["prepared", "unprepared", "copied", "copy ceased"] {
        assert!(
            seen.contains_key(what),
            "no game had a permanent {what}: {seen:?}"
        );
    }
}
