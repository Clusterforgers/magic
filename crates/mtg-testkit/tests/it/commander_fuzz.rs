//! Goal: whole bot games of Commander at 2 to 6 seats with 100-card singleton decks from the corpus finish and keep every invariant (the harness checks zones, life and the replay of the log), and they use the format: a commander is cast from the command zone and again for the tax, is put back into the command zone from graveyards, exile and hands, and is left where it is when its owner declines.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common::seeds::{for_each_seed, fuzz_games};

use std::sync::Mutex;

use mtg_engine::events::Event;
use mtg_engine::player::LossReason;
use mtg_engine::zones::Zone;
use mtg_engine::{GameSetup, GameState, apply_event};
use mtg_testkit::{commander_fuzz_setup, partner_fuzz_setup, play_game};
use rstest::rstest;

/// Games per table size; override with `MTG_FUZZ_GAMES`.
fn games() -> u64 {
    fuzz_games(8)
}

/// What a campaign of games did with the format.
#[derive(Default, Debug)]
struct Seen {
    cast_from_command: u64,
    cast_again: u64,
    returned_from_graveyard: u64,
    returned_from_exile: u64,
    returned_from_hand: u64,
    stayed: u64,
    lost_to_commander_damage: u64,
    /// The names of the commanders that were cast from a command zone.
    cast_names: std::collections::BTreeSet<String>,
}

impl Seen {
    /// Counts the format's moments of one game by folding its log over its setup.
    fn add(&mut self, setup: &GameSetup, events: &[Event]) {
        let mut state = GameState::from_setup(setup).unwrap();
        let mut casts = std::collections::BTreeMap::<_, u32>::new();
        for event in events {
            match event {
                Event::SpellCast { object, .. }
                    if state
                        .object(*object)
                        .is_some_and(|card| matches!(card.zone(), Zone::Command(_))) =>
                {
                    *casts.entry(*object).or_default() += 1;
                    if let Some(card) = state.object(*object) {
                        self.cast_names.insert(card.card().name.clone());
                    }
                }
                Event::ZoneChanged {
                    object,
                    from,
                    to: Zone::Command(_),
                    ..
                } if state.is_commander(*object) => match from {
                    Zone::Graveyard(_) => self.returned_from_graveyard += 1,
                    Zone::Exile => self.returned_from_exile += 1,
                    Zone::Hand(_) => self.returned_from_hand += 1,
                    _ => {}
                },
                Event::CommanderStayed { .. } => self.stayed += 1,
                Event::PlayerLost {
                    reason: LossReason::CommanderDamage,
                    ..
                } => self.lost_to_commander_damage += 1,
                _ => {}
            }
            apply_event(&mut state, event).unwrap();
        }
        self.cast_from_command += casts.len() as u64;
        self.cast_again += casts.values().filter(|n| **n >= 2).count() as u64;
    }
}

#[rstest]
#[case(2)]
#[case(3)]
#[case(4)]
#[case(6)]
fn seeded_commander_games_finish_and_keep_every_invariant(#[case] players: usize) {
    let seen = Mutex::new(Seen::default());
    for_each_seed(0..games(), |seed| {
        let setup = commander_fuzz_setup(players, seed).unwrap();
        let outcome = play_game(setup.clone(), seed).unwrap_or_else(|failure| panic!("{failure}"));
        seen.lock().unwrap().add(&setup, &outcome.events);
    });
    let seen = seen.into_inner().unwrap();
    eprintln!("{players} seats: {seen:?}");
    assert!(seen.cast_from_command > 0, "{seen:?}");
    // The seeds are fixed, so what a full campaign meets does not change.
    if games() >= 30 {
        assert!(seen.cast_again > 0, "no commander was cast twice: {seen:?}");
        assert!(
            seen.returned_from_graveyard > 0,
            "no commander came back from a graveyard: {seen:?}"
        );
        assert!(seen.stayed > 0, "no owner left a commander: {seen:?}");
    }
}

#[rstest]
#[case(2)]
#[case(3)]
#[case(4)]
#[case(6)]
fn seeded_games_of_partner_pairs_finish_and_keep_every_invariant(#[case] players: usize) {
    let seen = Mutex::new(Seen::default());
    for_each_seed(0..games(), |seed| {
        let setup = partner_fuzz_setup(players, seed).unwrap();
        let outcome = play_game(setup.clone(), seed).unwrap_or_else(|failure| panic!("{failure}"));
        seen.lock().unwrap().add(&setup, &outcome.events);
    });
    let seen = seen.into_inner().unwrap();
    eprintln!("{players} seats, pairs: {seen:?}");
    // Both partners of a seat are cast, each from the command zone.
    for name in ["First Partner", "Second Partner"] {
        assert!(
            seen.cast_names.contains(name),
            "{name} was never cast: {seen:?}"
        );
    }
}
