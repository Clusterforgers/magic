//! Goal: expend plays whole at tables of two, three and four seats: the corpus cards trigger once, on the cast whose mana takes their controller's turn total of mana spent on spells to N, not on the casts below or past it, and the log replays to the state it was played to.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::value::CounterKind;
use mtg_core::color::Color;
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::replay;
use mtg_testkit::cards::{creature, land};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::pid;

/// Seat 0 controls `watcher` and seven Mountains and holds creatures costing three, two and one mana.
fn w9a_table(seats: usize, watcher: Arc<CardDefinition>) -> Scenario {
    let mut lands: Vec<_> = (0..7).map(|_| ready(land(Color::Red))).collect();
    lands.push(ready(watcher));
    Scenario::new(seats)
        .hand(
            0,
            vec![
                creature("Three", 2, &[Color::Red], 3, 3),
                creature("Two", 1, &[Color::Red], 2, 2),
                creature("One", 0, &[Color::Red], 1, 1),
            ],
        )
        .battlefield(0, lands)
}

/// Passes priority and pays automatically until nothing is on the stack.
fn w9a_resolve(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..40 {
        match &driver.decision()?.kind {
            DecisionKind::Priority(_) if driver.state().stack().is_empty() => return Ok(()),
            DecisionKind::Priority(_) => {
                driver.pass()?;
            }
            DecisionKind::PayMana(_) => {
                driver.answer(Answer::PayMana(PayChoice::Auto))?;
            }
            other => panic!("unexpected {}", other.name()),
        }
    }
    panic!("the stack did not empty");
}

fn w9a_replays(driver: &Driver, scenario: Scenario) {
    let setup = scenario.setup().unwrap();
    let replayed = replay(&setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.engine().state_hash());
}

/// The life of each opponent of seat 0 and the +1/+1 counters on `watcher`.
fn w9a_seen(driver: &Driver, seats: usize, watcher: &str) -> (Vec<i32>, u32) {
    let state = driver.state();
    let lives = (1..seats)
        .map(|seat| state.player(pid(seat)).unwrap().life())
        .collect();
    let counters = state
        .objects()
        .find(|object| object.card().name == watcher)
        .and_then(|object| object.permanent())
        .map_or(0, |permanent| {
            permanent.counters_of(&CounterKind::PlusOnePlusOne)
        });
    (lives, counters)
}

#[rstest]
fn teapot_slinger_hits_each_opponent_once_as_the_fourth_mana_is_spent(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = w9a_table(seats, card("Teapot Slinger").unwrap());
    let mut driver = Driver::from_scenario(scenario.clone())?;
    let start = w9a_seen(&driver, seats, "Teapot Slinger").0;
    let lower: Vec<i32> = start.iter().map(|life| life - 2).collect();
    driver.cast("Three")?;
    w9a_resolve(&mut driver)?;
    assert_eq!(w9a_seen(&driver, seats, "Teapot Slinger").0, start);
    driver.cast("Two")?;
    w9a_resolve(&mut driver)?;
    assert_eq!(w9a_seen(&driver, seats, "Teapot Slinger").0, lower);
    driver.cast("One")?;
    w9a_resolve(&mut driver)?;
    assert_eq!(w9a_seen(&driver, seats, "Teapot Slinger").0, lower);
    w9a_replays(&driver, scenario);
    Ok(())
}

#[rstest]
fn wandertale_mentor_grows_once_whatever_the_order_of_the_casts(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = w9a_table(seats, card("Wandertale Mentor").unwrap());
    let mut driver = Driver::from_scenario(scenario.clone())?;
    driver.cast("Two")?;
    w9a_resolve(&mut driver)?;
    assert_eq!(w9a_seen(&driver, seats, "Wandertale Mentor").1, 0);
    driver.cast("Three")?;
    w9a_resolve(&mut driver)?;
    assert_eq!(w9a_seen(&driver, seats, "Wandertale Mentor").1, 1);
    driver.cast("One")?;
    w9a_resolve(&mut driver)?;
    assert_eq!(w9a_seen(&driver, seats, "Wandertale Mentor").1, 1);
    w9a_replays(&driver, scenario);
    Ok(())
}
