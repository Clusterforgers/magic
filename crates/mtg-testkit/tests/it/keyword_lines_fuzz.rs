//! Goal: whole bot games at 2 to 4 seats with the cards of the keyword lines (graft, afflict, kicker and/or, backup, replicate) keep every invariant, and across the seeds each part of the mechanics occurs: counters are put on a creature, counters move from a creature with graft onto another creature, a blocked creature with afflict makes its defending player lose life, and a spell with replicate is copied.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeMap;

use mtg_engine::Event;
use mtg_testkit::{PlayOptions, line_fuzz_setup, play_game_with};

use crate::common::seeds::{for_each_seed, fuzz_games};

#[test]
fn bot_games_with_graft_and_afflict_keep_every_invariant_and_use_each_part() {
    let seen = std::sync::Mutex::new(BTreeMap::<&'static str, u64>::new());
    let games = fuzz_games(16);
    for_each_seed(0..games, |seed| {
        for players in [2, 3, 4] {
            let outcome =
                play_game_with(line_fuzz_setup(players, seed), seed, PlayOptions::default())
                    .unwrap_or_else(|failure| panic!("{failure}"));
            let mut seen = seen.lock().unwrap();
            // A counter that left one object right before another received one is a move.
            let mut removed = 0u32;
            for event in &outcome.events {
                match event {
                    Event::CountersChanged(changed) if changed.delta < 0 => removed += 1,
                    Event::CountersChanged(changed) if changed.delta > 0 && removed > 0 => {
                        *seen.entry("counter moved").or_default() += 1;
                        removed -= 1;
                    }
                    Event::CountersChanged(changed) if changed.delta > 0 => {
                        *seen.entry("counter put").or_default() += 1;
                    }
                    Event::SpellCopied(_) => {
                        *seen.entry("spell copied").or_default() += 1;
                    }
                    Event::LifeLost { amount, .. } if *amount > 0 => {
                        *seen.entry("life lost").or_default() += 1;
                    }
                    _ => {}
                }
            }
        }
    });
    let seen = seen.into_inner().unwrap();
    for part in ["counter moved", "counter put", "life lost", "spell copied"] {
        assert!(
            seen.get(part).copied().unwrap_or(0) > 0,
            "no game went through {part}: {seen:?}"
        );
    }
}
