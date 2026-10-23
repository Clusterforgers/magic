//! Goal: whole bot games with decks of Gods, delirium creatures and quest enchantments finish and keep every invariant for many seeds at tables of two, three and four seats, and each mechanic occurs in a campaign (a God is a creature with enough devotion and an enchantment without it, a delirium creature is bigger than printed, a quest enchantment holds counters), and a seeded game is the same game every time.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Mutex;

use mtg_cards::value::CounterKind;
use mtg_core::card::CardType;
use mtg_engine::zones::Zone;
use mtg_engine::{GameState, characteristics, replay};
use mtg_testkit::conditional_static_cards::{DELIRIUM_CREATURES, GODS, QUESTS};
use mtg_testkit::{conditional_static_fuzz_setup, play_game};
use rstest::rstest;

use crate::common::seeds::{for_each_seed, fuzz_games};

/// Games per table size; override with `MTG_FUZZ_GAMES`.
fn games() -> u64 {
    fuzz_games(40)
}

/// Events between two states looked at in a game.
const STRIDE: usize = 120;

#[derive(Default)]
struct Seen {
    god_creatures: u64,
    god_enchantments: u64,
    grown_delirium: u64,
    quest_counters: u64,
}

impl Seen {
    /// What holds in the state: the permanents on the battlefield as the layers leave them.
    fn add(&mut self, game: &GameState) {
        for id in game.zone(Zone::Battlefield) {
            let (Some(object), Some(chars)) = (game.object(*id), characteristics(game, *id)) else {
                continue;
            };
            let name = object.card().name.as_str();
            if GODS.contains(&name) {
                if chars.types.contains(CardType::Creature) {
                    self.god_creatures += 1;
                } else {
                    self.god_enchantments += 1;
                }
            }
            if DELIRIUM_CREATURES.contains(&name)
                && chars.power > object.card().power
            {
                self.grown_delirium += 1;
            }
            if QUESTS.contains(&name)
                && object.permanent().is_some_and(|permanent| {
                    permanent.counters_of(&CounterKind::Other("quest".to_owned())) > 0
                })
            {
                self.quest_counters += 1;
            }
        }
    }
}

#[rstest]
#[case(2)]
#[case(3)]
#[case(4)]
fn seeded_conditional_static_games_finish_and_keep_every_invariant(#[case] players: usize) {
    let seen = Mutex::new(Seen::default());
    for_each_seed(0..games(), |seed| {
        let setup = conditional_static_fuzz_setup(players, seed).unwrap();
        let outcome = play_game(setup.clone(), seed).unwrap_or_else(|failure| panic!("{failure}"));
        let mut here = Seen::default();
        for end in (STRIDE..outcome.events.len()).step_by(STRIDE) {
            here.add(&replay(&setup, &outcome.events[..end]).unwrap());
        }
        here.add(&replay(&setup, &outcome.events).unwrap());
        let mut all = seen.lock().unwrap();
        all.god_creatures += here.god_creatures;
        all.god_enchantments += here.god_enchantments;
        all.grown_delirium += here.grown_delirium;
        all.quest_counters += here.quest_counters;
    });
    let seen = seen.into_inner().unwrap();
    let campaign = format!("{} games of {players} seats", games());
    assert!(
        seen.god_enchantments > 0,
        "no God stood without devotion in {campaign}"
    );
    assert!(
        seen.god_creatures > 0,
        "no God had enough devotion to be a creature in {campaign}"
    );
    assert!(
        seen.grown_delirium > 0,
        "no delirium creature was bigger than printed in {campaign}"
    );
    assert!(
        seen.quest_counters > 0,
        "no quest enchantment held a counter in {campaign}"
    );
}

#[test]
fn a_seeded_conditional_static_game_is_the_same_game_every_time() {
    let seed = 3;
    let setup = conditional_static_fuzz_setup(2, seed).unwrap();
    let first = play_game(setup.clone(), seed).unwrap_or_else(|failure| panic!("{failure}"));
    let second = play_game(setup, seed).unwrap_or_else(|failure| panic!("{failure}"));
    assert_eq!(first.final_hash, second.final_hash, "seed {seed}");
    assert_eq!(first.events, second.events, "seed {seed}");
}
