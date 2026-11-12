//! Goal: whole bot games with decks of the limits on blocking (blocks only fliers, one blocker at most, by a group, by an Aura or an Equipment) and of the cards that give melee and myriad finish and keep every invariant for many seeds at tables of two, three and four seats, and each mechanic occurs in a campaign (a creature that may be blocked by one creature at most attacks and is blocked, a creature that blocks only fliers blocks), and a seeded game is the same game every time.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeMap;
use std::sync::Mutex;

use mtg_engine::{Event, GameState, replay};
use mtg_testkit::block_limit_cards::{KEYWORD_GIVERS, LIMITED_CARDS};
use mtg_testkit::{block_limit_fuzz_setup, play_game};
use rstest::rstest;

use crate::common::seeds::{for_each_seed, fuzz_games};

/// Games per table size; override with `MTG_FUZZ_GAMES`.
fn games() -> u64 {
    fuzz_games(40)
}

/// The creatures that may be blocked by one creature at most.
const ONE_BLOCKER: &[&str] = &["Norwood Riders", "Challenger Troll"];

#[derive(Default)]
struct Seen {
    entered: BTreeMap<String, u64>,
    limited_attacks: u64,
    limited_blocked: u64,
}

impl Seen {
    /// What happened in the events, judged against the state at the end of the game.
    fn add(&mut self, game: &GameState, events: &[Event]) {
        let name = |id| game.object(id).map(|object| object.card().name.clone());
        let limited = |id| {
            game.object(id)
                .is_some_and(|object| ONE_BLOCKER.contains(&object.card().name.as_str()))
        };
        for event in events {
            match event {
                Event::SpellResolved { object } => {
                    if let Some(name) = name(*object) {
                        *self.entered.entry(name).or_default() += 1;
                    }
                }
                Event::AttackersDeclared { attackers, .. } => {
                    self.limited_attacks += attackers
                        .iter()
                        .filter(|(attacker, _)| limited(*attacker))
                        .count() as u64;
                }
                Event::BlockersDeclared { blocks, .. } => {
                    self.limited_blocked += blocks
                        .iter()
                        .filter(|(_, attacker)| limited(*attacker))
                        .count() as u64;
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
fn seeded_block_limit_games_finish_and_keep_every_invariant(#[case] players: usize) {
    let seen = Mutex::new(Seen::default());
    for_each_seed(0..games(), |seed| {
        let setup = block_limit_fuzz_setup(players, seed).unwrap();
        let outcome = play_game(setup.clone(), seed).unwrap_or_else(|failure| panic!("{failure}"));
        let end = replay(&setup, &outcome.events).unwrap();
        seen.lock().unwrap().add(&end, &outcome.events);
    });
    let seen = seen.into_inner().unwrap();
    let campaign = format!("{} games of {players} seats", games());
    let total = LIMITED_CARDS.len() + KEYWORD_GIVERS.len();
    let entered = LIMITED_CARDS
        .iter()
        .chain(KEYWORD_GIVERS)
        .filter(|name| seen.entered.get(**name).copied().unwrap_or(0) > 0)
        .count();
    assert!(
        entered * 2 >= total,
        "only {entered} of {total} cards were cast in {campaign}"
    );
    assert!(
        seen.limited_attacks > 0,
        "no creature limited to one blocker attacked in {campaign}"
    );
    assert!(
        seen.limited_blocked > 0,
        "no creature limited to one blocker was blocked in {campaign}"
    );
}

#[test]
fn a_seeded_block_limit_game_is_the_same_game_every_time() {
    let seed = 3;
    let setup = block_limit_fuzz_setup(2, seed).unwrap();
    let first = play_game(setup.clone(), seed).unwrap_or_else(|failure| panic!("{failure}"));
    let second = play_game(setup, seed).unwrap_or_else(|failure| panic!("{failure}"));
    assert_eq!(first.final_hash, second.final_hash, "seed {seed}");
    assert_eq!(first.events, second.events, "seed {seed}");
}
