//! Goal: whole bot games with decks of planeswalkers the compiler reads (Garruk, Primal Hunter, Freyalise, Llanowar's Fury, beside Jace Beleren and Garruk Wildspeaker) and of spells that exile themselves as they resolve (Seeds of Renewal, Teferi's Protection) finish and keep every invariant (zones, life, the journal, the replay of the log equal to the live state) for many seeds at tables of two, three and four seats, the games really use them (a loyalty ability is activated, a self-exiling spell ends in exile and never in the graveyard) and a seeded game is the same game every time.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Mutex;

use mtg_engine::zones::Zone;
use mtg_engine::{Event, replay};
use mtg_testkit::{play_game, walker_fuzz_setup};
use rstest::rstest;

use crate::common::seeds::{for_each_seed, fuzz_games};

/// Games per table size; override with `MTG_FUZZ_GAMES`.
fn games() -> u64 {
    fuzz_games(8)
}

#[derive(Default)]
struct Seen {
    loyalty_abilities: u64,
    self_exiled: u64,
}

#[rstest]
#[case(2)]
#[case(3)]
#[case(4)]
fn seeded_walker_games_finish_and_keep_every_invariant(#[case] players: usize) {
    let seen = Mutex::new(Seen::default());
    for_each_seed(0..games(), |seed| {
        let setup = walker_fuzz_setup(players, seed).unwrap();
        let outcome = play_game(setup.clone(), seed).unwrap_or_else(|failure| panic!("{failure}"));
        let end = replay(&setup, &outcome.events).unwrap();
        let mut seen = seen.lock().unwrap();
        for event in &outcome.events {
            match event {
                Event::AbilityActivated { source, .. }
                    if end.object(source.id).is_some_and(|walker| {
                        walker
                            .card()
                            .is_type(mtg_core::card::CardType::Planeswalker)
                    }) =>
                {
                    seen.loyalty_abilities += 1;
                }
                Event::ZoneChanged {
                    object,
                    from: Zone::Stack,
                    to: Zone::Exile,
                    ..
                } if end.object(*object).is_some_and(|spell| {
                    ["Seeds of Renewal", "Teferi's Protection"]
                        .contains(&spell.card().name.as_str())
                }) =>
                {
                    seen.self_exiled += 1;
                }
                _ => {}
            }
        }
    });
    let seen = seen.into_inner().unwrap();
    let campaign = format!("{} games of {players} seats", games());
    assert!(
        seen.loyalty_abilities > 0,
        "no loyalty ability was activated in {campaign}"
    );
    assert!(seen.self_exiled > 0, "no spell exiled itself in {campaign}");
}

#[test]
fn a_seeded_walker_game_is_the_same_game_every_time() {
    let seed = 3;
    let setup = walker_fuzz_setup(2, seed).unwrap();
    let first = play_game(setup.clone(), seed).unwrap_or_else(|failure| panic!("{failure}"));
    let second = play_game(setup, seed).unwrap_or_else(|failure| panic!("{failure}"));
    assert_eq!(first.final_hash, second.final_hash, "seed {seed}");
    assert_eq!(first.events, second.events, "seed {seed}");
}
