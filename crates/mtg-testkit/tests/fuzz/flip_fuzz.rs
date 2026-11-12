//! Goal: whole bot games at two, three and four seats with flip cards (a draw that fills a hand, a Snake that fills a board, ki counters from Spirit spells and their end step flip) keep every invariant, and across the seeds ki counters are put on, permanents are flipped, and a log replays to the same state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeMap;

use mtg_engine::Event;
use mtg_testkit::{PlayOptions, flip_fuzz_setup, play_game_with};

use crate::common::seeds::{for_each_seed, fuzz_games};

#[test]
fn bot_games_with_flip_cards_keep_every_invariant() {
    let seen = std::sync::Mutex::new(BTreeMap::<&'static str, u64>::new());
    let games = fuzz_games(15);
    for_each_seed(0..games, |seed| {
        for players in [2, 3, 4] {
            let outcome =
                play_game_with(flip_fuzz_setup(players, seed), seed, PlayOptions::default())
                    .unwrap_or_else(|failure| panic!("{failure}"));
            let replayed = mtg_engine::replay(&flip_fuzz_setup(players, seed), &outcome.events)
                .unwrap_or_else(|error| panic!("seed {seed}: the log does not replay: {error}"));
            assert_eq!(replayed.state_hash(), outcome.final_hash, "seed {seed}");
            let mut seen = seen.lock().unwrap();
            for event in &outcome.events {
                match event {
                    Event::Flipped { .. } => *seen.entry("flipped").or_default() += 1,
                    Event::CountersChanged(_) => *seen.entry("counters put on").or_default() += 1,
                    _ => {}
                }
            }
        }
    });
    let seen = seen.into_inner().unwrap();
    for what in ["flipped", "counters put on"] {
        assert!(
            seen.contains_key(what),
            "no game had a card {what}: {seen:?}"
        );
    }
}
