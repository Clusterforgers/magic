//! Goal: whole bot games at two, three and four seats with spells that take extra turns (for the caster, for a target player, two at once, with the untap step skipped), cards that add a combat phase and a main phase, and added beginning phases keep every invariant, and across the seeds extra turns are taken, phases are added, a turn begins without its untap step, a combat phase begins twice in one turn, and a log replays to the same state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeMap;

use mtg_engine::Event;
use mtg_engine::turn::Step;
use mtg_testkit::{PlayOptions, play_game_with, turn_fuzz_setup};

use crate::common::seeds::{for_each_seed, fuzz_games};

#[test]
fn bot_games_with_extra_turns_and_added_phases_keep_every_invariant() {
    let seen = std::sync::Mutex::new(BTreeMap::<&'static str, u64>::new());
    let games = fuzz_games(15);
    for_each_seed(0..games, |seed| {
        for players in [2, 3, 4] {
            let outcome =
                play_game_with(turn_fuzz_setup(players, seed), seed, PlayOptions::default())
                    .unwrap_or_else(|failure| panic!("{failure}"));
            let replayed = mtg_engine::replay(&turn_fuzz_setup(players, seed), &outcome.events)
                .unwrap_or_else(|error| panic!("seed {seed}: the log does not replay: {error}"));
            assert_eq!(replayed.state_hash(), outcome.final_hash, "seed {seed}");
            let mut seen = seen.lock().unwrap();
            let mut combats = 0;
            let mut after_turn = false;
            for event in &outcome.events {
                match event {
                    Event::TurnBegan { extra, .. } => {
                        combats = 0;
                        after_turn = true;
                        if *extra {
                            *seen.entry("an extra turn began").or_default() += 1;
                        }
                    }
                    Event::ExtraTurnSkipped { .. } => {
                        *seen.entry("an extra turn was passed over").or_default() += 1;
                    }
                    Event::PhasesAdded { .. } => {
                        *seen.entry("phases were added").or_default() += 1;
                    }
                    Event::StepBegan {
                        step: Step::BeginCombat,
                    } => {
                        combats += 1;
                        if combats == 2 {
                            *seen.entry("a second combat phase began").or_default() += 1;
                        }
                    }
                    Event::StepBegan { step: Step::Upkeep } if after_turn => {
                        *seen.entry("a turn began at its upkeep step").or_default() += 1;
                        after_turn = false;
                    }
                    Event::StepBegan { .. } => after_turn = false,
                    _ => {}
                }
            }
        }
    });
    let seen = seen.into_inner().unwrap();
    for what in [
        "an extra turn began",
        "phases were added",
        "a second combat phase began",
        "a turn began at its upkeep step",
        "an extra turn was passed over",
    ] {
        assert!(
            seen.contains_key(what),
            "no game had it that {what}: {seen:?}"
        );
    }
}
