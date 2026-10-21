//! Goal: whole bot games between creatures with the keywords of combat finish with a winner or a draw and keep every invariant (state-based actions settled at every decision, damage and life accounted for, the replayed log equal to the live state) for many seeds and table sizes, repeat exactly from their seed, and really use first strike, deathtouch and lifelink.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Mutex;

use mtg_engine::events::DeathCause;
use mtg_engine::turn::Step;
use mtg_engine::{Event, GameResult};
use mtg_testkit::{keyword_fuzz_setup, play_game};
use rstest::rstest;

use crate::common::seeds::for_each_seed;

/// Games per table size; override with `MTG_FUZZ_GAMES` to run longer or shorter campaigns.
fn games() -> u64 {
    std::env::var("MTG_FUZZ_GAMES")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(500)
}

/// What the games of a table size used of the keywords.
#[derive(Debug, Default)]
struct Used {
    first_strike_steps: u64,
    deathtouch_deaths: u64,
    lifelink_gains: u64,
    damage_events: u64,
}

#[rstest]
#[case(2)]
#[case(3)]
#[case(4)]
#[case(6)]
fn seeded_games_between_keyword_creatures_finish_and_keep_every_invariant(#[case] players: usize) {
    let totals = Mutex::new((0u64, Used::default()));
    for_each_seed(0..games(), |seed| {
        let outcome = play_game(keyword_fuzz_setup(players, seed), seed)
            .unwrap_or_else(|failure| panic!("{failure}"));
        let mut totals = totals.lock().unwrap();
        let (winners, used) = &mut *totals;
        if matches!(outcome.result, GameResult::Winner(_)) {
            *winners += 1;
        }
        for event in &outcome.events {
            match event {
                Event::StepBegan {
                    step: Step::FirstStrikeDamage,
                } => used.first_strike_steps += 1,
                Event::CreatureDied {
                    cause: DeathCause::Deathtouch,
                    ..
                } => used.deathtouch_deaths += 1,
                Event::LifeGained { .. } => used.lifelink_gains += 1,
                Event::DamageDealt { .. } => used.damage_events += 1,
                _ => {}
            }
        }
    });
    let (winners, used) = totals.into_inner().unwrap();
    assert!(winners > 0, "no game produced a winner out of {}", games());
    // A short campaign (MTG_FUZZ_GAMES) may not reach every keyword; the default one does.
    if games() >= 100 {
        assert!(used.first_strike_steps > 0, "{used:?}");
        assert!(used.deathtouch_deaths > 0, "{used:?}");
        assert!(used.lifelink_gains > 0, "{used:?}");
        assert!(used.damage_events > 0, "{used:?}");
    }
}

#[rstest]
#[case(2)]
#[case(4)]
fn a_keyword_game_repeats_exactly_from_its_seed(#[case] players: usize) {
    for seed in 0..20 {
        let first = play_game(keyword_fuzz_setup(players, seed), seed)
            .unwrap_or_else(|failure| panic!("{failure}"));
        let second = play_game(keyword_fuzz_setup(players, seed), seed)
            .unwrap_or_else(|failure| panic!("{failure}"));
        assert_eq!(first.final_hash, second.final_hash, "seed {seed}");
        assert_eq!(first.events, second.events, "seed {seed}");
    }
}
