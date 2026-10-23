//! Goal: whole bot games with decks of the static grants (landwalk, fear, exalted, ward, fabricate, annihilator, commander creatures, a subtype taken out of a base size, creature tokens during their controller's turn), Auras that remake a creature, bonuses counted by kinds or opponents, protection from a chosen quality finish and keep every invariant for many seeds at tables of two, three and four seats, each grant is held by some permanent in a campaign, and a seeded game is the same game every time.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeMap;
use std::sync::Mutex;

use mtg_engine::keywords::keywords_of;
use mtg_engine::zones::Zone;
use mtg_engine::{Event, GameState, replay};
use mtg_testkit::grant_cards::GRANT_CARDS;
use mtg_testkit::{grant_fuzz_setup, play_game};
use rstest::rstest;

use crate::common::seeds::{for_each_seed, fuzz_games};

/// Games per table size; override with `MTG_FUZZ_GAMES`.
fn games() -> u64 {
    fuzz_games(8)
}

#[derive(Default)]
struct Seen {
    entered: BTreeMap<String, u64>,
    held: BTreeMap<String, u64>,
}

impl Seen {
    /// What happened in the events, judged against the state at the end of the game.
    fn add(&mut self, game: &GameState, events: &[Event]) {
        for event in events {
            if let Event::ZoneChanged {
                object,
                to: Zone::Battlefield,
                ..
            }
            | Event::SpellResolved { object } = event
                && let Some(object) = game.object(*object)
            {
                *self.entered.entry(object.card().name.clone()).or_default() += 1;
            }
        }
        for &id in game.zone(Zone::Battlefield) {
            for keyword in keywords_of(game, id) {
                *self.held.entry(keyword.name().to_owned()).or_default() += 1;
            }
        }
    }
}

#[rstest]
#[case(2)]
#[case(3)]
#[case(4)]
fn seeded_grant_games_finish_and_keep_every_invariant(#[case] players: usize) {
    let seen = Mutex::new(Seen::default());
    for_each_seed(0..games(), |seed| {
        let setup = grant_fuzz_setup(players, seed).unwrap();
        let outcome = play_game(setup.clone(), seed).unwrap_or_else(|failure| panic!("{failure}"));
        let end = replay(&setup, &outcome.events).unwrap();
        seen.lock().unwrap().add(&end, &outcome.events);
    });
    let seen = seen.into_inner().unwrap();
    let entered = GRANT_CARDS
        .iter()
        .filter(|name| seen.entered.get(**name).copied().unwrap_or(0) > 0)
        .count();
    assert!(
        entered * 2 >= GRANT_CARDS.len(),
        "only {entered} of {} cards entered play in {} games of {players} seats",
        GRANT_CARDS.len(),
        games()
    );
}

#[test]
fn a_campaign_holds_the_granted_keywords() {
    let mut seen = Seen::default();
    for seed in 0..96 {
        let setup = grant_fuzz_setup(2, seed).unwrap();
        let outcome = play_game(setup.clone(), seed).unwrap_or_else(|failure| panic!("{failure}"));
        let end = replay(&setup, &outcome.events).unwrap();
        seen.add(&end, &outcome.events);
    }
    for keyword in [
        "Ward",
        "Landwalk",
        "Fear",
        "Extort",
        "Fabricate",
        "Annihilator",
        "Protection",
    ] {
        assert!(
            seen.held.get(keyword).copied().unwrap_or(0) > 0,
            "no permanent held {keyword} at the end of a game"
        );
    }
}

#[test]
fn a_seeded_grant_game_is_the_same_game_every_time() {
    let seed = 3;
    let setup = grant_fuzz_setup(2, seed).unwrap();
    let first = play_game(setup.clone(), seed).unwrap_or_else(|failure| panic!("{failure}"));
    let second = play_game(setup, seed).unwrap_or_else(|failure| panic!("{failure}"));
    assert_eq!(first.final_hash, second.final_hash, "seed {seed}");
    assert_eq!(first.events, second.events, "seed {seed}");
}
