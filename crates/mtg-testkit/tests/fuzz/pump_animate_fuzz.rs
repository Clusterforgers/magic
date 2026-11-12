//! Goal: whole bot games with decks of the cards that change a group or a target for a while (shrinks of all but one creature type, base sizes for a turn and until the next turn, lost abilities and a new look, grants to several targets, lands and artifacts that become creatures) finish and keep every invariant (zones, life, the journal, effects listed in the order of their timestamps and none past its duration, the replay of the log equal to the live state) for many seeds at tables of two, three and four seats, and the games really use them: base sizes are set, abilities are lost, creature types are set, types are added, keywords are granted, effects expire, and a seeded game is the same game every time.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common::seeds::fuzz_games;
use mtg_engine::Event;
use mtg_engine::effects::{Modification, Selector};
use mtg_testkit::{change_fuzz_setup, play_game};
use rstest::rstest;

/// Games per table size; override with `MTG_FUZZ_GAMES`.
fn games() -> u64 {
    fuzz_games(20)
}

#[derive(Default)]
struct Seen {
    base_sizes: u64,
    lost_abilities: u64,
    creature_types: u64,
    added_types: u64,
    keywords: u64,
    groups: u64,
    expired: u64,
}

impl Seen {
    fn add(&mut self, events: &[Event]) {
        for event in events {
            match event {
                Event::EffectCreated { effect } => {
                    match &effect.modification {
                        Modification::SetPowerToughness { .. } => self.base_sizes += 1,
                        Modification::LoseAbilities => self.lost_abilities += 1,
                        Modification::SetCreatureTypes(_) => self.creature_types += 1,
                        Modification::AddType(_) => self.added_types += 1,
                        Modification::AddKeyword(_) => self.keywords += 1,
                        _ => {}
                    }
                    if matches!(&effect.selector, Selector::Objects(named) if named.len() > 1) {
                        self.groups += 1;
                    }
                }
                Event::EffectExpired { .. } => self.expired += 1,
                _ => {}
            }
        }
    }
}

#[rstest]
#[case(2)]
#[case(3)]
#[case(4)]
fn seeded_change_games_finish_and_keep_every_invariant(#[case] players: usize) {
    let mut seen = Seen::default();
    for seed in 0..games() {
        let setup = change_fuzz_setup(players, seed).unwrap();
        let outcome = play_game(setup, seed).unwrap_or_else(|failure| panic!("{failure}"));
        seen.add(&outcome.events);
    }
    let campaign = format!("{} games of {players} seats", games());
    assert!(seen.base_sizes > 0, "no base size was set in {campaign}");
    assert!(
        seen.lost_abilities > 0,
        "no abilities were lost in {campaign}"
    );
    assert!(
        seen.creature_types > 0,
        "no creature types were set in {campaign}"
    );
    assert!(seen.added_types > 0, "no type was added in {campaign}");
    assert!(seen.keywords > 0, "no keyword was granted in {campaign}");
    assert!(seen.groups > 0, "no group was changed in {campaign}");
    assert!(seen.expired > 0, "no effect ran out in {campaign}");
}

#[test]
fn a_seeded_change_game_is_the_same_game_every_time() {
    for seed in [3, 17, 91] {
        let first = play_game(change_fuzz_setup(3, seed).unwrap(), seed).unwrap();
        let second = play_game(change_fuzz_setup(3, seed).unwrap(), seed).unwrap();
        assert_eq!(first.events, second.events);
        assert_eq!(first.final_hash, second.final_hash);
    }
}
