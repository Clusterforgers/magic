//! Goal: whole bot games with decks of creatures whose power and toughness count something and of permanents that change the life gained finish and keep every invariant (zones, life, the journal, effects in the order of their timestamps, no creature left that state-based actions would destroy, the replay of the log equal to the live state) for many seeds at tables of two, three and four seats, the games really use them (half of the cards enter play, a creature with its stats stays on the battlefield, life is gained), and a seeded game is the same game every time.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeMap;
use std::sync::Mutex;

use mtg_engine::characteristics::characteristics;
use mtg_engine::zones::Zone;
use mtg_engine::{Event, replay};
use mtg_testkit::defining_cards::DEFINING_CARDS;
use mtg_testkit::{defining_fuzz_setup, play_game};
use rstest::rstest;

use crate::common::seeds::{for_each_seed, fuzz_games};

/// Games per table size; override with `MTG_FUZZ_GAMES`. A creature that counts cards in graveyards makes a game slow, so a campaign is short.
fn games() -> u64 {
    fuzz_games(6)
}

#[derive(Default)]
struct Seen {
    entered: BTreeMap<String, u64>,
    life_gained: u64,
}

impl Seen {
    fn add(&mut self, game: &mtg_engine::GameState, events: &[Event]) {
        for event in events {
            match event {
                Event::ZoneChanged {
                    object,
                    to: Zone::Battlefield,
                    ..
                }
                | Event::SpellResolved { object } => {
                    if let Some(object) = game.object(*object) {
                        *self.entered.entry(object.card().name.clone()).or_default() += 1;
                    }
                }
                Event::LifeGained { .. } => self.life_gained += 1,
                _ => {}
            }
        }
    }
}

#[rstest]
#[case(2)]
#[case(3)]
#[case(4)]
fn seeded_defining_games_finish_and_keep_every_invariant(#[case] players: usize) {
    let seen = Mutex::new(Seen::default());
    for_each_seed(0..games(), |seed| {
        let setup = defining_fuzz_setup(players, seed).unwrap();
        let outcome = play_game(setup.clone(), seed).unwrap_or_else(|failure| panic!("{failure}"));
        let end = replay(&setup, &outcome.events).unwrap();
        seen.lock().unwrap().add(&end, &outcome.events);
    });
    let seen = seen.into_inner().unwrap();
    let campaign = format!("{} games of {players} seats", games());
    // One copy of each card is in a deck, so a short campaign sees most of them and not each.
    let entered = DEFINING_CARDS
        .iter()
        .filter(|name| seen.entered.get(**name).copied().unwrap_or(0) > 0)
        .count();
    assert!(
        entered * 2 >= DEFINING_CARDS.len(),
        "only {entered} of {} cards entered play in {campaign}",
        DEFINING_CARDS.len()
    );
    assert!(seen.life_gained > 0, "no life was gained in {campaign}");
}

#[test]
fn a_creature_defined_by_a_count_ends_a_game_with_its_stats_on_the_battlefield() {
    let (players, mut sized) = (2, 0u64);
    for seed in 0..4 {
        let setup = defining_fuzz_setup(players, seed).unwrap();
        let outcome = play_game(setup.clone(), seed).unwrap_or_else(|failure| panic!("{failure}"));
        let end = replay(&setup, &outcome.events).unwrap();
        for &id in end.zone(Zone::Battlefield) {
            let object = end.object(id).unwrap();
            if DEFINING_CARDS.contains(&object.card().name.as_str()) {
                let shown = characteristics(&end, id).unwrap();
                sized += u64::from(shown.power.is_some() && shown.toughness.is_some());
            }
        }
    }
    assert!(
        sized > 0,
        "no defining creature ended a game on the battlefield"
    );
}

#[test]
fn a_seeded_defining_game_is_the_same_game_every_time() {
    let seed = 3;
    let setup = defining_fuzz_setup(2, seed).unwrap();
    let first = play_game(setup.clone(), seed).unwrap_or_else(|failure| panic!("{failure}"));
    let second = play_game(setup, seed).unwrap_or_else(|failure| panic!("{failure}"));
    assert_eq!(first.final_hash, second.final_hash, "seed {seed}");
    assert_eq!(first.events, second.events, "seed {seed}");
}
