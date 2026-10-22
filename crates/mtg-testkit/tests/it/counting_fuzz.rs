//! Goal: whole bot games with decks of spells that cost less for what is counted or for where they are cast from, and of permanents whose group enters tapped, finish and keep every invariant (zones, life, the journal, effects in the order of their timestamps, the replay of the log equal to the live state) for many seeds at tables of two, three and four seats, the games really use them (some of the cards are cast or enter play) and a seeded game is the same game every time.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeMap;
use std::sync::Mutex;

use mtg_engine::{Event, replay};
use mtg_testkit::counting_cards::COUNTING_CARDS;
use mtg_testkit::{counting_fuzz_setup, play_game};
use rstest::rstest;

use crate::common::seeds::{for_each_seed, fuzz_games};

/// Games per table size; override with `MTG_FUZZ_GAMES`.
fn games() -> u64 {
    fuzz_games(8)
}

#[derive(Default)]
struct Seen {
    cast: BTreeMap<String, u64>,
}

impl Seen {
    fn add(&mut self, game: &mtg_engine::GameState, events: &[Event]) {
        for event in events {
            match event {
                Event::SpellResolved { object } => {
                    if let Some(object) = game.object(*object) {
                        *self.cast.entry(object.card().name.clone()).or_default() += 1;
                    }
                }
                _ => {}
            }
        }
    }
}

#[rstest]
#[case(2)]
#[case(3)]
#[case(4)]
fn seeded_counting_games_finish_and_keep_every_invariant(#[case] players: usize) {
    let seen = Mutex::new(Seen::default());
    for_each_seed(0..games(), |seed| {
        let setup = counting_fuzz_setup(players, seed).unwrap();
        let outcome = play_game(setup.clone(), seed).unwrap_or_else(|failure| panic!("{failure}"));
        let end = replay(&setup, &outcome.events).unwrap();
        seen.lock().unwrap().add(&end, &outcome.events);
    });
    let seen = seen.into_inner().unwrap();
    let campaign = format!("{} games of {players} seats", games());
    // One copy of each card is in a deck and most of them cost twelve, so a short campaign casts only some.
    let used = COUNTING_CARDS
        .iter()
        .filter(|name| seen.cast.get(**name).copied().unwrap_or(0) > 0)
        .count();
    assert!(used > 0, "no card of the list was cast in {campaign}");
}

#[test]
fn a_seeded_counting_game_is_the_same_game_every_time() {
    let seed = 3;
    let setup = counting_fuzz_setup(2, seed).unwrap();
    let first = play_game(setup.clone(), seed).unwrap_or_else(|failure| panic!("{failure}"));
    let second = play_game(setup, seed).unwrap_or_else(|failure| panic!("{failure}"));
    assert_eq!(first.final_hash, second.final_hash, "seed {seed}");
    assert_eq!(first.events, second.events, "seed {seed}");
}
