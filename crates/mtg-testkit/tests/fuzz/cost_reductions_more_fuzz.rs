//! Goal: whole bot games with decks of spells and permanents that cost less with a target, a tally of the turn, one opponent's lands, graveyard or life, a kind of spell, the chosen creature type or the creatures attacking the caster finish and keep every invariant (zones, life, the journal, effects in the order of their timestamps, the replay of the log equal to the live state) for many seeds at tables of two, three and four seats, the reductions really apply (some spell is cast for less mana than its mana value) and a seeded game is the same game every time.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeMap;
use std::sync::Mutex;

use mtg_engine::{Event, replay};
use mtg_testkit::reduction_cards::MORE_REDUCTION_CARDS;
use mtg_testkit::{more_reduction_fuzz_setup, play_game};
use rstest::rstest;

use crate::common::seeds::{for_each_seed, fuzz_games};

/// Games per table size; override with `MTG_FUZZ_GAMES`.
fn games() -> u64 {
    fuzz_games(8)
}

#[derive(Default)]
struct Seen {
    cast: BTreeMap<String, u64>,
    /// Spells cast for less mana than their mana value, by card.
    cheaper: BTreeMap<String, u64>,
}

impl Seen {
    fn add(&mut self, game: &mtg_engine::GameState, events: &[Event]) {
        for event in events {
            let Event::SpellCast {
                object, choices, ..
            } = event
            else {
                continue;
            };
            let Some(spell) = game.object(*object) else {
                continue;
            };
            let name = spell.card().name.clone();
            *self.cast.entry(name.clone()).or_default() += 1;
            let paid = u32::try_from(choices.spent().len()).unwrap_or(u32::MAX);
            if choices.x.is_none()
                && choices.way.is_none()
                && !choices.spent().is_empty()
                && paid < spell.card().mana_value()
            {
                *self.cheaper.entry(name).or_default() += 1;
            }
        }
    }
}

#[rstest]
#[case(2)]
#[case(3)]
#[case(4)]
fn seeded_games_with_targeted_and_tallied_reductions_keep_every_invariant(#[case] players: usize) {
    let seen = Mutex::new(Seen::default());
    for_each_seed(0..games(), |seed| {
        let setup = more_reduction_fuzz_setup(players, seed).unwrap();
        let outcome = play_game(setup.clone(), seed).unwrap_or_else(|failure| panic!("{failure}"));
        let end = replay(&setup, &outcome.events).unwrap();
        seen.lock().unwrap().add(&end, &outcome.events);
    });
    let seen = seen.into_inner().unwrap();
    let campaign = format!("{} games of {players} seats", games());
    let used = MORE_REDUCTION_CARDS
        .iter()
        .filter(|name| seen.cast.get(**name).copied().unwrap_or(0) > 0)
        .count();
    assert!(used > 0, "no card of the list was cast in {campaign}");
    let cheaper: u64 = seen.cheaper.values().sum();
    assert!(
        cheaper > 0,
        "no spell was cast for less than its mana value in {campaign}: {:?}",
        seen.cast
    );
}

#[test]
fn a_seeded_game_with_targeted_and_tallied_reductions_is_the_same_game_every_time() {
    let seed = 3;
    let setup = more_reduction_fuzz_setup(2, seed).unwrap();
    let first = play_game(setup.clone(), seed).unwrap_or_else(|failure| panic!("{failure}"));
    let second = play_game(setup, seed).unwrap_or_else(|failure| panic!("{failure}"));
    assert_eq!(first.final_hash, second.final_hash, "seed {seed}");
    assert_eq!(first.events, second.events, "seed {seed}");
}
