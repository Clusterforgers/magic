//! Goal: whole bot games with decks of the block restrictions and the granted mana abilities finish and keep every invariant for many seeds at tables of two, three and four seats, and each mechanic occurs in a campaign (a restricted creature attacks and is blocked, a creature that has no mana ability of its own is tapped for mana), and a seeded game is the same game every time.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeMap;
use std::sync::Mutex;

use mtg_core::card::CardType;
use mtg_engine::{Event, GameState, replay};
use mtg_testkit::block_restriction_cards::{MANA_GRANTERS, RESTRICTED_CREATURES};
use mtg_testkit::{block_restriction_fuzz_setup, play_game};
use rstest::rstest;

use crate::common::seeds::{for_each_seed, fuzz_games};

/// Games per table size; override with `MTG_FUZZ_GAMES`.
fn games() -> u64 {
    fuzz_games(10)
}

#[derive(Default)]
struct Seen {
    entered: BTreeMap<String, u64>,
    restricted_attacks: u64,
    restricted_blocked: u64,
    creature_mana: u64,
}

impl Seen {
    /// What happened in the events, judged against the state at the end of the game.
    fn add(&mut self, game: &GameState, events: &[Event]) {
        let name = |id| game.object(id).map(|object| object.card().name.clone());
        let restricted = |id| {
            game.object(id).is_some_and(|object| {
                RESTRICTED_CREATURES.contains(&object.card().name.as_str())
                    || object.card().subtypes.iter().any(|kind| kind.0 == "Sliver")
            })
        };
        for event in events {
            match event {
                Event::SpellResolved { object } => {
                    if let Some(name) = name(*object) {
                        *self.entered.entry(name).or_default() += 1;
                    }
                }
                Event::AttackersDeclared { attackers, .. } => {
                    self.restricted_attacks += attackers
                        .iter()
                        .filter(|(attacker, _)| restricted(*attacker))
                        .count() as u64;
                }
                Event::BlockersDeclared { blocks, .. } => {
                    self.restricted_blocked += blocks
                        .iter()
                        .filter(|(_, attacker)| restricted(*attacker))
                        .count() as u64;
                }
                Event::ManaAdded {
                    source: Some(source),
                    ..
                } if game.object(*source).is_some_and(|object| {
                    object.card().types.contains(&CardType::Creature)
                        && object.card().abilities.is_empty()
                }) =>
                {
                    self.creature_mana += 1;
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
fn seeded_block_restriction_games_finish_and_keep_every_invariant(#[case] players: usize) {
    let seen = Mutex::new(Seen::default());
    for_each_seed(0..games(), |seed| {
        let setup = block_restriction_fuzz_setup(players, seed).unwrap();
        let outcome = play_game(setup.clone(), seed).unwrap_or_else(|failure| panic!("{failure}"));
        let end = replay(&setup, &outcome.events).unwrap();
        seen.lock().unwrap().add(&end, &outcome.events);
    });
    let seen = seen.into_inner().unwrap();
    let campaign = format!("{} games of {players} seats", games());
    let cards = RESTRICTED_CREATURES.iter().chain(MANA_GRANTERS);
    let total = RESTRICTED_CREATURES.len() + MANA_GRANTERS.len();
    let entered = cards
        .filter(|name| seen.entered.get(**name).copied().unwrap_or(0) > 0)
        .count();
    assert!(
        entered * 2 >= total,
        "only {entered} of {total} cards were cast in {campaign}"
    );
    assert!(
        seen.restricted_attacks > 0,
        "no restricted creature attacked in {campaign}"
    );
    assert!(
        seen.restricted_blocked > 0,
        "no restricted creature was blocked in {campaign}"
    );
    assert!(
        seen.creature_mana > 0,
        "no creature was tapped for a granted mana ability in {campaign}"
    );
}

#[test]
fn a_seeded_block_restriction_game_is_the_same_game_every_time() {
    let seed = 3;
    let setup = block_restriction_fuzz_setup(2, seed).unwrap();
    let first = play_game(setup.clone(), seed).unwrap_or_else(|failure| panic!("{failure}"));
    let second = play_game(setup, seed).unwrap_or_else(|failure| panic!("{failure}"));
    assert_eq!(first.final_hash, second.final_hash, "seed {seed}");
    assert_eq!(first.events, second.events, "seed {seed}");
}
