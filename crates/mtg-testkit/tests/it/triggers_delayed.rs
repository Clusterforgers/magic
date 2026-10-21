//! Goal: an effect that sets up a delayed triggered ability (CR 603.7) makes one that waits for the next event it names: at the next end step, once and only once; one that is not "once" lasts the turn and runs out with it; it outlives the source that set it up; and its log replays to the same state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use common::pid;
use mtg_engine::turn::Step;
use mtg_engine::{Event, replay};
use mtg_testkit::cards::{burn, free_creature, gain_life, mountain};
use mtg_testkit::trigger_cards::{creature_with, delaying, hourglass, vigil, when};
use mtg_testkit::{Driver, Scenario, ScriptError, ready};

fn gains(events: &[Event]) -> Vec<u32> {
    events
        .iter()
        .filter_map(|event| match event {
            Event::LifeGained { amount, .. } => Some(*amount),
            _ => None,
        })
        .collect()
}

fn count(events: &[Event], wanted: fn(&Event) -> bool) -> usize {
    events.iter().filter(|e| wanted(e)).count()
}

#[test]
fn a_delayed_trigger_waits_for_the_end_step_triggers_once_and_is_gone() -> Result<(), ScriptError> {
    let mut driver =
        Driver::from_scenario(Scenario::new(2).battlefield(0, [ready(hourglass("Hourglass", 2))]))?;
    let mut events = driver.begin_activation("Hourglass")?;
    while !driver.state().stack().is_empty() {
        events.extend(driver.pass()?);
    }
    assert_eq!(driver.state().delayed().len(), 1, "it is waiting");
    assert_eq!(gains(&events), [] as [u32; 0], "nothing yet");

    // On to the end step: the ability triggers as the step begins and goes on the stack.
    driver.pass_until(Step::End)?;
    assert_eq!(driver.state().stack().len(), 1, "it is on the stack");
    assert_eq!(driver.state().delayed().len(), 0, "and no longer waiting");
    let mut later = Vec::new();
    while !driver.state().stack().is_empty() {
        later.extend(driver.pass()?);
    }
    assert_eq!(gains(&later), [2]);
    driver.expect_life(0, 22)?;

    // The next end steps find nothing to trigger.
    driver.idle_until(3, Step::End)?;
    driver.expect_life(0, 22)?;
    assert_eq!(driver.state().stack(), []);
    Ok(())
}

#[test]
fn a_delayed_trigger_that_is_not_once_triggers_every_time_this_turn_and_runs_out_with_it()
-> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(2)
            .hand(0, [burn("Bolt", 3), burn("Spark", 3)])
            .battlefield(
                0,
                [
                    ready(mountain()),
                    ready(mountain()),
                    ready(vigil("Vigil", 1)),
                ],
            )
            .battlefield(
                1,
                [
                    ready(free_creature("A", 1, 1)),
                    ready(free_creature("B", 1, 1)),
                    ready(free_creature("C", 1, 1)),
                ],
            ),
    )?;
    let mut events = driver.begin_activation("Vigil")?;
    while !driver.state().stack().is_empty() {
        events.extend(driver.pass()?);
    }
    for (spell, victim) in [("Bolt", "A"), ("Spark", "B")] {
        let at = driver.target_named(victim)?;
        events.extend(driver.cast_at(spell, &[at])?);
        while !driver.state().stack().is_empty() {
            events.extend(driver.pass()?);
        }
    }
    assert_eq!(gains(&events), [1, 1], "once for each creature that died");
    assert_eq!(driver.state().delayed().len(), 1, "it stays for the turn");

    // The turn passes; it runs out, and a creature dying in the next one gives nothing.
    let mut after = Vec::new();
    for _ in 0..40 {
        if driver.state().turn().number() > 1 {
            break;
        }
        after.extend(driver.answer_passively()?);
    }
    assert_eq!(driver.state().delayed().len(), 0);
    assert_eq!(
        count(&after, |e| matches!(e, Event::DelayedTriggerExpired { .. })),
        1,
        "{after:?}"
    );
    Ok(())
}

#[test]
fn a_delayed_trigger_outlives_the_source_that_set_it_up() -> Result<(), ScriptError> {
    let mage = creature_with(
        "Mage",
        vec![delaying(when::an_end_step(), true, vec![gain_life(5)])],
    );
    let mut driver = Driver::from_scenario(
        Scenario::new(2)
            .at(1, 1, Step::Main1)
            .hand(1, [burn("Bolt", 3)])
            .battlefield(0, [ready(mage)])
            .battlefield(1, [ready(mountain())]),
    )?;
    // The Mage's controller sets the trigger up in the other player's turn, then the Mage is burned.
    driver.pass()?;
    let mut events = driver.begin_activation("Mage")?;
    driver.pass()?;
    driver.pass()?;
    assert_eq!(driver.state().delayed().len(), 1);
    driver.pass()?;
    driver.pass()?;
    let at = driver.target_named("Mage")?;
    events.extend(driver.cast_at("Bolt", &[at])?);
    while !driver.state().stack().is_empty() {
        events.extend(driver.pass()?);
    }
    assert!(driver.id_of("Mage").is_err(), "the Mage is dead");
    driver.pass_until(Step::End)?;
    let mut later = Vec::new();
    while !driver.state().stack().is_empty() {
        later.extend(driver.pass()?);
    }
    assert_eq!(gains(&later), [5], "the ability does what it was set up to");
    driver.expect_life(0, 25)?;
    Ok(())
}

#[test]
fn the_log_of_a_game_with_delayed_triggers_replays_to_the_same_state() -> Result<(), ScriptError> {
    let scenario = || Scenario::new(2).battlefield(0, [ready(hourglass("Hourglass", 2))]);
    let mut driver = Driver::from_scenario(scenario())?;
    driver.begin_activation("Hourglass")?;
    for _ in 0..3 {
        driver.pass()?;
    }
    let live = driver.state().state_hash();
    // Mid-game and, after the end step, once it has triggered and resolved.
    let replayed = replay(&scenario().setup().unwrap(), driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), live);
    assert_eq!(replayed.delayed().len(), 1);
    driver.pass_until(Step::End)?;
    while !driver.state().stack().is_empty() {
        driver.pass()?;
    }
    let live = driver.state().state_hash();
    let replayed = replay(&scenario().setup().unwrap(), driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), live);
    assert_eq!(replayed.delayed().len(), 0);
    let _ = pid(0);
    Ok(())
}
