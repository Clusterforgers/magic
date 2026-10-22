//! Goal: whole bot games with decks of the group statics (creatures that enter tapped, a keyword for the creatures with a counter, ward for the other creatures, a skipped draw step) finish and keep every invariant for many seeds at tables of two, three and four seats, and each mechanic occurs in a campaign (a creature enters tapped under a foe's static ability, a turn goes by without a draw step, a keyword is held through a counter, ward is held), and a seeded game is the same game every time.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeMap;
use std::sync::Mutex;

use mtg_engine::keywords::keywords_of;
use mtg_engine::zones::Zone;
use mtg_engine::{Event, GameState, replay};
use mtg_testkit::group_static_cards::GROUP_STATIC_CARDS;
use mtg_testkit::{group_static_fuzz_setup, play_game};
use rstest::rstest;

use crate::common::seeds::{for_each_seed, fuzz_games};

/// Games per table size; override with `MTG_FUZZ_GAMES`.
fn games() -> u64 {
    fuzz_games(8)
}

#[derive(Default)]
struct Seen {
    entered: BTreeMap<String, u64>,
    entered_tapped_creature: u64,
    turns_without_a_draw_step: u64,
    ward_held: u64,
    trample_held: u64,
}

impl Seen {
    /// What happened in the events, judged against the state at the end of the game.
    fn add(&mut self, game: &GameState, events: &[Event]) {
        let mut steps_of_turn: Vec<mtg_engine::turn::Step> = Vec::new();
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
                Event::Entered {
                    object,
                    tapped: true,
                    ..
                } if game.object(*object).is_some_and(|o| {
                    o.card().types.contains(&mtg_core::card::CardType::Creature)
                }) =>
                {
                    self.entered_tapped_creature += 1;
                }
                Event::TurnBegan { .. } => steps_of_turn.clear(),
                Event::StepBegan { step } => {
                    steps_of_turn.push(*step);
                    if *step == mtg_engine::turn::Step::Main1
                        && !steps_of_turn.contains(&mtg_engine::turn::Step::Draw)
                    {
                        self.turns_without_a_draw_step += 1;
                    }
                }
                _ => {}
            }
        }
        for &id in game.zone(Zone::Battlefield) {
            for keyword in keywords_of(game, id) {
                match keyword.name() {
                    "Ward" => self.ward_held += 1,
                    "Trample" => self.trample_held += 1,
                    _ => {}
                }
            }
        }
    }
}

#[rstest]
#[case(2)]
#[case(3)]
#[case(4)]
fn seeded_group_static_games_finish_and_keep_every_invariant(#[case] players: usize) {
    let seen = Mutex::new(Seen::default());
    for_each_seed(0..games(), |seed| {
        let setup = group_static_fuzz_setup(players, seed).unwrap();
        let outcome = play_game(setup.clone(), seed).unwrap_or_else(|failure| panic!("{failure}"));
        let end = replay(&setup, &outcome.events).unwrap();
        seen.lock().unwrap().add(&end, &outcome.events);
    });
    let seen = seen.into_inner().unwrap();
    let campaign = format!("{} games of {players} seats", games());
    let entered = GROUP_STATIC_CARDS
        .iter()
        .filter(|name| seen.entered.get(**name).copied().unwrap_or(0) > 0)
        .count();
    assert!(
        entered * 2 >= GROUP_STATIC_CARDS.len(),
        "only {entered} of {} cards entered play in {campaign}",
        GROUP_STATIC_CARDS.len()
    );
    assert!(
        seen.entered_tapped_creature > 0,
        "no creature entered tapped under a static ability in {campaign}"
    );
    assert!(
        seen.turns_without_a_draw_step > 0,
        "no turn went by without a draw step in {campaign}"
    );
}

#[test]
fn a_campaign_holds_ward_and_trample_through_the_statics() {
    let mut seen = Seen::default();
    for seed in 0..16 {
        let setup = group_static_fuzz_setup(2, seed).unwrap();
        let outcome = play_game(setup.clone(), seed).unwrap_or_else(|failure| panic!("{failure}"));
        let end = replay(&setup, &outcome.events).unwrap();
        seen.add(&end, &outcome.events);
    }
    assert!(
        seen.ward_held > 0,
        "no creature held ward at the end of a game"
    );
    assert!(
        seen.trample_held > 0,
        "no creature held trample at the end of a game"
    );
}

#[test]
fn a_seeded_group_static_game_is_the_same_game_every_time() {
    let seed = 3;
    let setup = group_static_fuzz_setup(2, seed).unwrap();
    let first = play_game(setup.clone(), seed).unwrap_or_else(|failure| panic!("{failure}"));
    let second = play_game(setup, seed).unwrap_or_else(|failure| panic!("{failure}"));
    assert_eq!(first.final_hash, second.final_hash, "seed {seed}");
    assert_eq!(first.events, second.events, "seed {seed}");
}
