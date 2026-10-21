//! Goal: whole bot games with decks of spells and activated abilities finish and keep every invariant (zones, life, damage, the journal, zone-change counters and replay equal to the live state), for many seeds and table sizes, and the games really use the pipeline: spells resolve, abilities go on the stack and cease to exist, and some spells fizzle.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_engine::Event;
use mtg_engine::events::ResolutionOutcome;
use mtg_testkit::{PlayOptions, play_game, play_game_with, spell_fuzz_setup};
use rstest::rstest;

/// Games per table size; override with `MTG_FUZZ_GAMES` to run longer or shorter campaigns.
fn games() -> u64 {
    std::env::var("MTG_FUZZ_GAMES")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(500)
}

#[derive(Default)]
struct Seen {
    resolutions: u64,
    abilities: u64,
    fizzled: u64,
    damage: u64,
    life: u64,
    drawn: u64,
}

impl Seen {
    fn add(&mut self, events: &[Event]) {
        for event in events {
            match event {
                Event::ResolutionBegan { .. } => self.resolutions += 1,
                Event::AbilityPutOnStack(_) => self.abilities += 1,
                Event::ResolutionEnded {
                    outcome: ResolutionOutcome::Fizzled,
                    ..
                } => self.fizzled += 1,
                Event::DamageDealt { .. } => self.damage += 1,
                Event::LifeGained { .. } | Event::LifeLost { .. } => self.life += 1,
                Event::CardDrawn { .. } => self.drawn += 1,
                _ => {}
            }
        }
    }
}

#[rstest]
#[case(2)]
#[case(3)]
#[case(4)]
fn seeded_spell_games_finish_and_keep_every_invariant(#[case] players: usize) {
    let mut seen = Seen::default();
    for seed in 0..games() {
        let outcome = play_game(spell_fuzz_setup(players, seed), seed)
            .unwrap_or_else(|failure| panic!("{failure}"));
        seen.add(&outcome.events);
    }
    assert!(seen.resolutions > 0, "no spell or ability ever resolved");
    assert!(seen.abilities > 0, "no ability was ever activated");
    assert!(
        seen.fizzled > 0,
        "no spell ever fizzled in {} games",
        games()
    );
    assert!(seen.damage > 0 && seen.life > 0 && seen.drawn > 0);
}

#[rstest]
#[case(2)]
#[case(4)]
fn the_same_games_finish_when_the_engine_finishes_the_payments(#[case] players: usize) {
    let options = PlayOptions {
        partial_payments: true,
        ..PlayOptions::default()
    };
    for seed in 0..games() / 5 {
        play_game_with(spell_fuzz_setup(players, seed), seed, options)
            .unwrap_or_else(|failure| panic!("{failure}"));
    }
}

#[test]
fn a_seeded_spell_game_is_the_same_game_every_time() {
    for seed in [3, 17, 91] {
        let first = play_game(spell_fuzz_setup(2, seed), seed).unwrap();
        let second = play_game(spell_fuzz_setup(2, seed), seed).unwrap();
        assert_eq!(first.events, second.events);
        assert_eq!(first.final_hash, second.final_hash);
    }
}
