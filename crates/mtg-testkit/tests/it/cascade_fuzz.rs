//! Goal: whole bot games at two, three and four seats with cascade and discover cards (Bloodbraid Elf, a creature that discovers) keep every invariant, and across the seeds spells are cast for nothing during a resolution with X 0, cards are bottomed in a random order.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeMap;

use mtg_cards::value::CastWay;
use mtg_engine::Event;
use mtg_testkit::{PlayOptions, cascade_fuzz_setup, play_game_with};

use crate::common::seeds::{for_each_seed, fuzz_games};

#[test]
fn bot_games_with_cascade_cards_keep_every_invariant_and_cast_during_resolutions() {
    let seen = std::sync::Mutex::new(BTreeMap::<&'static str, u64>::new());
    let games = fuzz_games(20);
    for_each_seed(0..games, |seed| {
        for players in [2, 3, 4] {
            let outcome = play_game_with(
                cascade_fuzz_setup(players, seed),
                seed,
                PlayOptions::default(),
            )
            .unwrap_or_else(|failure| panic!("{failure}"));
            let mut seen = seen.lock().unwrap();
            let mut resolving = false;
            for event in &outcome.events {
                match event {
                    Event::ResolutionBegan { .. } => resolving = true,
                    Event::ResolutionEnded { .. } => resolving = false,
                    Event::SpellCast { choices, .. } if resolving => {
                        assert_eq!(choices.way, Some(CastWay::Free));
                        assert_eq!(choices.x.unwrap_or(0), 0, "an alternative cost has no X");
                        *seen.entry("cast during a resolution").or_default() += 1;
                    }
                    Event::LibraryBottomed { random: true, .. } => {
                        *seen.entry("bottomed at random").or_default() += 1;
                    }
                    _ => {}
                }
            }
        }
    });
    let seen = seen.into_inner().unwrap();
    for what in ["cast during a resolution", "bottomed at random"] {
        assert!(
            seen.get(what).copied().unwrap_or(0) > 0,
            "never seen: {what}: {seen:?}"
        );
    }
}
