//! Goal: whole bot games at two, three and four seats with a companion at every seat keep every invariant, and across the seeds each companion is revealed once before the first shuffle and brought into the hand at most once, always by its owner, during their own main phase, and a brought companion is a card of its owner's in the game.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::{BTreeMap, BTreeSet};

use mtg_core::ids::PlayerId;
use mtg_engine::Event;
use mtg_engine::turn::Step;
use mtg_testkit::{PlayOptions, companion_fuzz_setup, play_game_with};

use crate::common::seeds::{for_each_seed, fuzz_games};

#[test]
fn bot_games_with_a_companion_at_every_seat_keep_every_invariant() {
    let seen = std::sync::Mutex::new(BTreeMap::<&'static str, u64>::new());
    let games = fuzz_games(20);
    for_each_seed(0..games, |seed| {
        for players in [2, 3, 4] {
            let outcome = play_game_with(
                companion_fuzz_setup(players, seed),
                seed,
                PlayOptions::default(),
            )
            .unwrap_or_else(|failure| panic!("{failure}"));
            let mut seen = seen.lock().unwrap();
            let mut revealed: BTreeSet<PlayerId> = BTreeSet::new();
            let mut brought: BTreeSet<PlayerId> = BTreeSet::new();
            let mut shuffled = false;
            let mut active = PlayerId::from_index(0);
            let mut step = Step::Untap;
            for event in &outcome.events {
                match event {
                    Event::LibraryShuffled { .. } => shuffled = true,
                    Event::CompanionRevealed { player } => {
                        assert!(!shuffled, "revealed after a shuffle");
                        assert!(revealed.insert(*player), "revealed twice");
                        *seen.entry("revealed").or_default() += 1;
                    }
                    Event::TurnBegan { active: next, .. } => active = *next,
                    Event::StepBegan { step: next } => step = *next,
                    Event::CompanionBrought { player, .. } => {
                        assert!(revealed.contains(player), "brought before it was revealed");
                        assert!(brought.insert(*player), "brought twice");
                        assert_eq!(*player, active, "brought on another player's turn");
                        assert!(step.is_main(), "brought in {step:?}");
                        *seen.entry("brought").or_default() += 1;
                    }
                    _ => {}
                }
            }
            assert_eq!(revealed.len(), players, "every seat reveals its companion");
        }
    });
    let seen = seen.into_inner().unwrap();
    for what in ["revealed", "brought"] {
        assert!(
            seen.get(what).copied().unwrap_or(0) > 0,
            "never seen: {what}: {seen:?}"
        );
    }
}
