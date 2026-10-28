//! Goal: "When you next cast an instant or sorcery spell this turn, copy that spell" (Doublecast, Galvanic Iteration, Teach by Example) sets up a delayed trigger that copies the next such spell its controller casts this turn once, at tables of two to four seats: a creature spell or an opponent's spell does not use it up, a second spell is not copied, it runs out with the turn, and the log replays to the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use common::copying::{game, keep_targets_until_done, life, replays, run};
use mtg_engine::turn::Step;
use mtg_testkit::{Driver, ScriptError};
use rstest::rstest;

/// Seat 0 casts `setup` and lets it resolve, then casts Lightning Bolt at seat 1: the trigger copies it.
fn bolt_after(seats: usize, setup: &str) -> Result<(Driver, mtg_engine::GameSetup), ScriptError> {
    let scenario = || game(seats, &[setup, "Lightning Bolt", "Lightning Bolt"], 3, 6);
    let setup_state = scenario().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario())?;
    driver.cast(setup)?;
    run(&mut driver, seats)?;
    assert_eq!(
        driver.state().delayed().len(),
        1,
        "{setup} waits for a cast"
    );
    let foe = driver.target_player(1);
    driver.cast_at("Lightning Bolt", &[foe])?;
    Ok((driver, setup_state))
}

#[rstest]
fn the_next_instant_is_copied_once(
    #[values("Doublecast", "Galvanic Iteration", "Teach by Example")] setup: &str,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let (mut driver, setup_state) = bolt_after(seats, setup)?;
    keep_targets_until_done(&mut driver, seats)?;
    assert_eq!(life(&driver, 1), 20 - 6, "the Bolt and its copy");
    let foe = driver.target_player(1);
    driver.cast_at("Lightning Bolt", &[foe])?;
    assert!(
        driver.state().delayed().is_empty(),
        "the trigger was used up"
    );
    keep_targets_until_done(&mut driver, seats)?;
    assert_eq!(life(&driver, 1), 20 - 9);
    replays(&driver, &setup_state);
    Ok(())
}

#[rstest]
fn the_trigger_runs_out_with_the_turn(#[values(2, 3, 4)] seats: usize) -> Result<(), ScriptError> {
    let scenario = || game(seats, &["Doublecast"], 0, 2);
    let setup_state = scenario().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario())?;
    driver.cast("Doublecast")?;
    run(&mut driver, seats)?;
    let state = driver.state();
    assert!(!state.delayed()[0].lapsed(state), "it waits this turn");
    driver.idle_until(2, Step::Upkeep)?;
    let state = driver.state();
    assert!(
        state.delayed().iter().all(|waiting| waiting.lapsed(state)),
        "nothing waits past the turn"
    );
    replays(&driver, &setup_state);
    Ok(())
}
