//! Goal: whole bot games with decks of cards that trigger finish and keep every invariant (zones, life, damage, the journal, zone-change counters, no trigger waiting while a player has priority, and the replay of the log equal to the live state), for many seeds and table sizes, and the games really use triggers: abilities trigger, wait and go on the stack with their targets, some are removed for want of a target, several of one player are ordered, an intervening "if" lapses, delayed triggers are set up and run out; a seeded game is the same game every time.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Mutex;

use mtg_engine::Event;
use mtg_engine::events::ResolutionOutcome;
use mtg_testkit::{PlayOptions, play_game, play_game_with, trigger_fuzz_setup};
use rstest::rstest;

use crate::common::seeds::for_each_seed;

/// Games per table size; override with `MTG_FUZZ_GAMES` to run longer or shorter campaigns. A game of triggers lasts
/// about three times as many turns as one of vanilla creatures, so there are fewer of them.
fn games() -> u64 {
    std::env::var("MTG_FUZZ_GAMES")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(200)
}

#[derive(Default)]
struct Seen {
    triggered: u64,
    put_on_the_stack: u64,
    with_targets: u64,
    removed: u64,
    ordered: u64,
    delayed_set_up: u64,
    delayed_ran_out: u64,
    resolved: u64,
    fizzled: u64,
    lapsed: u64,
}

impl Seen {
    fn add(&mut self, events: &[Event]) {
        for event in events {
            match event {
                Event::AbilityTriggered(_) => self.triggered += 1,
                Event::AbilityPutOnStack(put) if put.cause.is_some() => {
                    self.put_on_the_stack += 1;
                    self.with_targets += u64::from(!put.choices.targets.is_empty());
                }
                Event::TriggerRemoved { .. } => self.removed += 1,
                Event::TriggersOrdered { .. } => self.ordered += 1,
                Event::DelayedTriggerCreated(_) => self.delayed_set_up += 1,
                Event::DelayedTriggerExpired { .. } => self.delayed_ran_out += 1,
                Event::ResolutionEnded { outcome, .. } => match outcome {
                    ResolutionOutcome::Resolved => self.resolved += 1,
                    ResolutionOutcome::Fizzled => self.fizzled += 1,
                    ResolutionOutcome::Lapsed => self.lapsed += 1,
                    ResolutionOutcome::Countered => {}
                },
                _ => {}
            }
        }
    }
}

#[rstest]
#[case(2)]
#[case(3)]
#[case(4)]
fn seeded_trigger_games_finish_and_keep_every_invariant(#[case] players: usize) {
    let seen = Mutex::new(Seen::default());
    for_each_seed(0..games(), |seed| {
        let outcome = play_game(trigger_fuzz_setup(players, seed), seed)
            .unwrap_or_else(|failure| panic!("{failure}"));
        seen.lock().unwrap().add(&outcome.events);
    });
    let seen = seen.into_inner().unwrap();
    let campaign = format!("{} games of {players} seats", games());
    assert!(seen.triggered > 0, "nothing triggered in {campaign}");
    assert!(
        seen.put_on_the_stack > 0 && seen.put_on_the_stack <= seen.triggered,
        "triggers went on the stack: {} of {}",
        seen.put_on_the_stack,
        seen.triggered
    );
    assert!(
        seen.with_targets > 0,
        "no trigger chose targets in {campaign}"
    );
    assert!(seen.removed > 0, "no trigger was removed in {campaign}");
    assert!(seen.ordered > 0, "no player ordered triggers in {campaign}");
    assert!(seen.delayed_set_up > 0, "no delayed trigger in {campaign}");
    assert!(seen.resolved > 0);
    assert!(seen.fizzled > 0, "no trigger fizzled in {campaign}");
    assert!(seen.lapsed > 0, "no intervening if lapsed in {campaign}");
    assert!(
        seen.delayed_ran_out > 0,
        "no delayed trigger ran out in {campaign}"
    );
}

#[rstest]
#[case(2)]
#[case(4)]
fn the_same_games_finish_when_the_engine_finishes_the_payments(#[case] players: usize) {
    let options = PlayOptions {
        partial_payments: true,
        ..PlayOptions::default()
    };
    for_each_seed(0..games() / 5, |seed| {
        play_game_with(trigger_fuzz_setup(players, seed), seed, options)
            .unwrap_or_else(|failure| panic!("{failure}"));
    });
}

#[test]
fn a_seeded_trigger_game_is_the_same_game_every_time() {
    for seed in [3, 17, 91] {
        let first = play_game(trigger_fuzz_setup(3, seed), seed).unwrap();
        let second = play_game(trigger_fuzz_setup(3, seed), seed).unwrap();
        assert_eq!(first.events, second.events);
        assert_eq!(first.final_hash, second.final_hash);
    }
}
