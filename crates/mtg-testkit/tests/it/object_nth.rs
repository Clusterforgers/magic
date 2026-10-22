//! Goal: Seedglaive Mentor grows the first time each turn it becomes the target of a spell or ability its controller controls and no other time (not for another creature, not for an opponent's spell, not the second time, again the next turn), at two, three and four seats; and whole bot games at those sizes with decks of creatures that count their attacks, targets and damage finish, keep every invariant, trigger every one of them and replay to the same log and state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;
use crate::common::seeds::fuzz_games;

use std::collections::BTreeSet;

use common::pid;
use mtg_cards::keyword::Keyword;
use mtg_cards::value::CounterKind;
use mtg_engine::choose::YesNoChoice;
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::turn::Step;
use mtg_engine::{Event, GameSetup, replay};
use mtg_testkit::cards::{free_creature, mountain, pinger};
use mtg_testkit::corpus::card;
use mtg_testkit::nth_cards::{MENTOR, NTH_CARDS};
use mtg_testkit::pump_cards::grant;
use mtg_testkit::{Driver, Scenario, ScriptError, nth_fuzz_setup, play_game, ready};
use rstest::rstest;

fn counters(driver: &Driver, name: &str) -> u32 {
    let id = driver.id_of(name).unwrap();
    driver
        .state()
        .object(id)
        .and_then(|object| object.permanent())
        .map_or(0, |permanent| {
            permanent.counters_of(&CounterKind::PlusOnePlusOne)
        })
}

fn assert_replays(driver: &Driver) {
    let state = replay(driver.engine().setup(), driver.engine().events()).unwrap();
    assert_eq!(state.state_hash(), driver.state().state_hash());
}

/// Passes priority while the stack holds something.
fn settle(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..60 {
        match &driver.decision()?.kind {
            DecisionKind::YesNo(_) => {
                driver.answer(Answer::YesNo(YesNoChoice(true)))?;
            }
            DecisionKind::Priority(_) if !driver.state().stack().is_empty() => {
                driver.pass()?;
            }
            _ => return Ok(()),
        }
    }
    Err(ScriptError::Stuck("the stack".into()))
}

/// Something that targets a permanent.
#[derive(Debug, Clone, Copy)]
enum Act {
    /// Seat 0 casts one of its Rushes at the permanent.
    MySpell(&'static str),
    /// Seat 0 taps its Pinger at the permanent.
    MyAbility(&'static str),
    /// Seat 1 casts its Rush at the permanent when seat 0 passes priority.
    TheirSpell(&'static str),
}

/// Seat 0 controls Seedglaive Mentor, a Bear, a Pinger and three Mountains and holds two Rushes; seat 1 controls a Mountain
/// and holds a Rush. The other seats have nothing.
fn table(seats: usize) -> Result<Driver, ScriptError> {
    let rush = |name: &str| grant(name, Keyword::Haste);
    Driver::from_scenario(
        Scenario::new(seats)
            .hand(0, [rush("Rush A"), rush("Rush B")])
            .hand(1, [rush("Their Rush")])
            .battlefield(
                0,
                [
                    ready(card(MENTOR).unwrap()),
                    ready(free_creature("Bear", 2, 2)),
                    ready(pinger("Pinger", 0)),
                    ready(mountain()),
                    ready(mountain()),
                    ready(mountain()),
                ],
            )
            .battlefield(1, [ready(mountain())]),
    )
}

fn act(driver: &mut Driver, act: Act, spells: &mut usize) -> Result<(), ScriptError> {
    match act {
        Act::MySpell(at) => {
            let name = ["Rush A", "Rush B"][*spells];
            *spells += 1;
            let at = driver.target_named(at)?;
            driver.cast_at(name, &[at])?;
        }
        Act::MyAbility(at) => {
            driver.begin_activation("Pinger")?;
            let at = driver.target_named(at)?;
            driver.choose_targets(&[at])?;
        }
        Act::TheirSpell(at) => {
            driver.pass()?;
            assert_eq!(driver.decision()?.player, pid(1), "seat 1 has priority");
            let at = driver.target_named(at)?;
            driver.cast_at("Their Rush", &[at])?;
        }
    }
    settle(driver)
}

#[rstest]
#[case::my_spell(&[Act::MySpell("Seedglaive Mentor")], 1)]
#[case::my_ability(&[Act::MyAbility("Seedglaive Mentor")], 1)]
#[case::not_the_second_spell(&[Act::MySpell("Seedglaive Mentor"), Act::MySpell("Seedglaive Mentor")], 1)]
#[case::not_the_second_whatever_it_is(&[Act::MySpell("Seedglaive Mentor"), Act::MyAbility("Seedglaive Mentor")], 1)]
#[case::an_ability_then_a_spell(&[Act::MyAbility("Seedglaive Mentor"), Act::MySpell("Seedglaive Mentor")], 1)]
#[case::another_creature_is_not_it(&[Act::MySpell("Bear")], 0)]
#[case::another_creature_does_not_use_it_up(&[Act::MySpell("Bear"), Act::MySpell("Seedglaive Mentor")], 1)]
#[case::an_opponents_spell_is_not_mine(&[Act::TheirSpell("Seedglaive Mentor")], 0)]
#[case::an_opponents_does_not_use_it_up(&[Act::TheirSpell("Seedglaive Mentor"), Act::MySpell("Seedglaive Mentor")], 1)]
#[case::mine_does_not_use_up_an_opponents(&[Act::MySpell("Seedglaive Mentor"), Act::TheirSpell("Seedglaive Mentor")], 1)]
fn seedglaive_mentor_grows_the_first_time_each_turn_it_is_targeted_by_what_its_controller_controls(
    #[case] acts: &[Act],
    #[case] grown: u32,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = table(seats)?;
    let mut spells = 0;
    for next in acts {
        act(&mut driver, *next, &mut spells)?;
    }
    assert_eq!(counters(&driver, MENTOR), grown);
    assert_replays(&driver);
    Ok(())
}

#[rstest]
fn seedglaive_mentor_grows_again_on_the_next_turn(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = table(seats)?;
    let mut spells = 0;
    act(&mut driver, Act::MySpell(MENTOR), &mut spells)?;
    assert_eq!(counters(&driver, MENTOR), 1);
    // A whole round of turns later it is seat 0's turn again, and the count has started from zero.
    let turn = u32::try_from(seats).unwrap() + 1;
    driver.idle_until(turn, Step::Main1)?;
    act(&mut driver, Act::MySpell(MENTOR), &mut spells)?;
    assert_eq!(counters(&driver, MENTOR), 2);
    act(&mut driver, Act::MyAbility(MENTOR), &mut spells)?;
    assert_eq!(counters(&driver, MENTOR), 2, "the second time of the turn");
    assert_replays(&driver);
    Ok(())
}

/// The cards a game's triggers came from, by name.
fn triggered_cards(setup: &GameSetup, events: &[Event]) -> BTreeSet<String> {
    let state = replay(setup, events).unwrap();
    events
        .iter()
        .filter_map(|event| match event {
            Event::AbilityTriggered(triggered) => state.object(triggered.source.id),
            _ => None,
        })
        .map(|object| object.card().name.clone())
        .collect()
}

fn games() -> u64 {
    fuzz_games(30)
}

#[rstest]
#[case(2)]
#[case(3)]
#[case(4)]
fn seeded_nth_games_finish_keep_every_invariant_and_trigger_every_card(#[case] players: usize) {
    let games = games();
    let mut seen = BTreeSet::new();
    for seed in 0..games {
        let setup = nth_fuzz_setup(players, seed).unwrap();
        let outcome = play_game(setup.clone(), seed).unwrap_or_else(|failure| panic!("{failure}"));
        seen.extend(triggered_cards(&setup, &outcome.events));
    }
    for name in NTH_CARDS.iter().copied().chain([MENTOR]) {
        assert!(
            seen.contains(name),
            "{name} never triggered in {games} games of {players} seats: {seen:?}"
        );
    }
}

#[rstest]
#[case(2)]
#[case(3)]
#[case(4)]
fn a_seeded_nth_game_is_the_same_game_every_time_and_replays_to_its_state(#[case] players: usize) {
    for seed in 0..6u64 {
        let setup = nth_fuzz_setup(players, seed).unwrap();
        let first = play_game(setup.clone(), seed).unwrap_or_else(|failure| panic!("{failure}"));
        let second = play_game(setup.clone(), seed).unwrap_or_else(|failure| panic!("{failure}"));
        assert_eq!(first.events, second.events, "seed {seed}");
        assert_eq!(first.final_hash, second.final_hash, "seed {seed}");
        let state = replay(&setup, &first.events).unwrap();
        assert_eq!(state.state_hash(), first.final_hash, "seed {seed}");
    }
}
