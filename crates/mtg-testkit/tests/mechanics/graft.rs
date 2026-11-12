//! Goal: graft plays whole at tables of two, three and four seats as its rules say: the permanent enters with its counters, another creature entering under any seat's control lets its controller move a +1/+1 counter onto it (asked, and declined without a move), nothing is asked once no counter is left or when the grafter itself enters, two grafters feed each other, and the log replays to the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common::pid;
use mtg_cards::keyword::Keyword;
use mtg_cards::value::CounterKind;
use mtg_engine::choose::YesNoChoice;
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::turn::Step;
use mtg_engine::{Engine, GameSetup, replay};
use mtg_testkit::cards::{free_creature, keyworded_creature};
use mtg_testkit::{Driver, Scenario, ScriptError};
use rstest::rstest;

/// A free 1/1 with graft `count`.
fn grafter(name: &str, count: u32) -> std::sync::Arc<mtg_cards::CardDefinition> {
    keyworded_creature(name, 0, &[], 1, 1, &[Keyword::Graft(count)])
}

/// Seat 0 holds these cards in the first main phase of turn 1; every other seat holds a Foe creature that costs nothing.
fn table(seats: usize, hand: &[&str]) -> Scenario {
    let mut cards = vec![
        grafter("Grafter", 3),
        grafter("Single", 1),
        free_creature("Newcomer", 1, 1),
    ];
    cards.retain(|card| hand.contains(&card.name.as_str()));
    let mut scenario = Scenario::new(seats).at(1, 0, Step::Main1).hand(0, cards);
    for seat in 1..seats {
        scenario = scenario.hand(seat, [free_creature("Foe", 1, 1)]);
    }
    scenario
}

/// The +1/+1 counters on the permanent called `name`.
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

/// Casts `name`, passes until the stack is empty again and answers each question of graft with `accept`; returns how many
/// were asked.
fn cast_and_settle(driver: &mut Driver, name: &str, accept: bool) -> Result<usize, ScriptError> {
    driver.cast(name)?;
    let mut asked = 0;
    for _ in 0..12 {
        match &driver.decision()?.kind {
            DecisionKind::YesNo(_) => {
                asked += 1;
                driver.answer(Answer::YesNo(YesNoChoice(accept)))?;
            }
            DecisionKind::Priority(_) if driver.state().stack().is_empty() => return Ok(asked),
            _ => {
                driver.pass()?;
            }
        }
    }
    panic!("the stack did not settle");
}

/// The log replays to the live state, and an engine rebuilt from it stands at the same decision.
fn assert_replays(driver: &Driver, setup: &GameSetup) {
    let log = driver.engine().events().to_vec();
    let replayed = replay(setup, &log).unwrap();
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
    let resumed = Engine::resume(setup.clone(), log).unwrap();
    assert_eq!(resumed.state_hash(), driver.state().state_hash());
    assert_eq!(resumed.decision(), driver.engine().decision());
}

#[rstest]
fn a_grafter_enters_with_its_counters_and_moves_one_to_the_next_creature_if_its_controller_agrees(
    #[values(2, 3, 4)] seats: usize,
    #[values(true, false)] accept: bool,
) -> Result<(), ScriptError> {
    let setup = table(seats, &["Grafter", "Newcomer"]).setup()?;
    let mut driver = Driver::from_scenario(table(seats, &["Grafter", "Newcomer"]))?;
    assert_eq!(cast_and_settle(&mut driver, "Grafter", accept)?, 0);
    assert_eq!(counters(&driver, "Grafter"), 3);
    assert_eq!(cast_and_settle(&mut driver, "Newcomer", accept)?, 1);
    let moved = u32::from(accept);
    assert_eq!(counters(&driver, "Grafter"), 3 - moved);
    assert_eq!(counters(&driver, "Newcomer"), moved);
    assert_replays(&driver, &setup);
    Ok(())
}

#[rstest]
fn the_counters_run_out_after_as_many_moves_as_the_grafter_has(
    #[values(2, 3)] seats: usize,
) -> Result<(), ScriptError> {
    let hand = vec![
        grafter("Single", 1),
        free_creature("Newcomer", 1, 1),
        free_creature("Late", 1, 1),
    ];
    let mut driver =
        Driver::from_scenario(Scenario::new(seats).at(1, 0, Step::Main1).hand(0, hand))?;
    cast_and_settle(&mut driver, "Single", true)?;
    assert_eq!(counters(&driver, "Single"), 1);
    assert_eq!(cast_and_settle(&mut driver, "Newcomer", true)?, 1);
    assert_eq!(counters(&driver, "Single"), 0);
    // With the grafter empty nothing is asked of the next creature.
    assert_eq!(cast_and_settle(&mut driver, "Late", true)?, 0);
    assert_eq!(counters(&driver, "Late"), 0);
    Ok(())
}

#[rstest]
fn another_seats_creature_entering_is_offered_the_counter_too(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table(seats, &["Grafter"]))?;
    cast_and_settle(&mut driver, "Grafter", true)?;
    // Seat 0 passes through the turn until the next seat's turn, where it casts its Foe.
    driver.idle_until(2, Step::Main1)?;
    let foe = driver.decision()?.player;
    assert_eq!(foe, pid(1));
    assert_eq!(cast_and_settle(&mut driver, "Foe", true)?, 1);
    assert_eq!(counters(&driver, "Grafter"), 2);
    assert_eq!(counters(&driver, "Foe"), 1);
    Ok(())
}

#[test]
fn two_grafters_feed_each_other_and_a_grafter_never_feeds_itself() -> Result<(), ScriptError> {
    let first = grafter("First", 2);
    let second = grafter("Second", 2);
    let mut driver = Driver::from_scenario(
        Scenario::new(2)
            .at(1, 0, Step::Main1)
            .hand(0, [first, second]),
    )?;
    assert_eq!(cast_and_settle(&mut driver, "First", true)?, 0);
    assert_eq!(cast_and_settle(&mut driver, "Second", true)?, 1);
    assert_eq!(counters(&driver, "First"), 1);
    assert_eq!(counters(&driver, "Second"), 3);
    Ok(())
}
