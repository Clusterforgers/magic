//! Goal: delayed triggers that one batch of simultaneous events sets up (a shield that exiles each creature a wipe destroys and returns it at the end step) each get an id of their own at every table size, so the game goes on, each creature returns, the other seats' creatures stay dead and the log replays.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use common::removal::corpus;
use common::{name_of, pid};
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_engine::{Event, replay};
use mtg_testkit::cards::{plains, swamp};
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// The names of the nonland permanents `seat` controls, sorted.
fn controlled(driver: &Driver, seat: usize) -> Vec<String> {
    let state = driver.state();
    let mut names: Vec<String> = state
        .zone(Zone::Battlefield)
        .iter()
        .filter(|id| {
            state
                .object(**id)
                .is_some_and(|o| o.controller() == pid(seat))
        })
        .map(|id| name_of(state, *id))
        .filter(|name| name != "Plains" && name != "Swamp")
        .collect();
    names.sort();
    names
}

/// The ids of the delayed triggers the log set up, in order.
fn delayed_ids(events: &[Event]) -> Vec<u64> {
    events
        .iter()
        .filter_map(|event| match event {
            Event::DelayedTriggerCreated(created) => Some(created.id.0),
            _ => None,
        })
        .collect()
}

/// Answers every question passively and passes until `done` holds.
fn play_until(
    driver: &mut Driver,
    what: &str,
    done: impl Fn(&Driver) -> bool,
) -> Result<(), ScriptError> {
    for _ in 0..300 {
        if done(driver) {
            return Ok(());
        }
        if driver.decision()?.kind.name() == "priority" {
            driver.pass()?;
        } else {
            driver.answer_passively()?;
        }
    }
    Err(ScriptError::Stuck(what.to_owned()))
}

/// Resolves the stack.
fn settle(driver: &mut Driver) -> Result<(), ScriptError> {
    play_until(driver, "the stack", |driver| {
        driver.state().stack().is_empty()
            && driver
                .decision()
                .is_ok_and(|decision| decision.kind.name() == "priority")
    })
}

/// Seat 0 casts Cosmic Intervention, then `wipe` destroys its `count` creatures and the other seats' Grizzly Bears:
/// each of seat 0's is exiled with a return of its own and comes back at the end step.
#[rstest]
fn a_shield_exiles_each_creature_of_a_wipe_with_its_own_return(
    #[values(2, 3, 4)] seats: usize,
    #[values("Wrath of God", "Day of Judgment", "Bontu's Last Reckoning")] wipe: &str,
    #[values(2, 3)] count: usize,
) -> Result<(), ScriptError> {
    let creatures = ["Grizzly Bears", "Llanowar Elves", "Serra Angel"];
    let mut permanents: Vec<_> = (0..8).map(|_| ready(plains())).collect();
    permanents.extend((0..3).map(|_| ready(swamp())));
    permanents.extend(creatures[..count].iter().map(|name| ready(corpus(name))));
    let mut scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .hand_named(0, ["Cosmic Intervention", wipe])
        .battlefield(0, permanents);
    for seat in 1..seats {
        scenario = scenario.battlefield_named(seat, ["Grizzly Bears"]);
    }
    let mut driver = Driver::from_scenario(scenario)?;
    let before = controlled(&driver, 0);
    driver.cast("Cosmic Intervention")?;
    settle(&mut driver)?;
    driver.cast(wipe)?;
    settle(&mut driver)?;
    assert!(controlled(&driver, 0).is_empty(), "the wipe took them");
    assert_eq!(
        driver.state().zone(Zone::Exile).len(),
        count,
        "the shield exiled each"
    );
    let ids = delayed_ids(driver.engine().events());
    assert_eq!(ids.len(), count, "a return for each");
    let mut distinct = ids.clone();
    distinct.sort_unstable();
    distinct.dedup();
    assert_eq!(distinct, ids, "each return has an id of its own");
    play_until(&mut driver, "the next turn", |driver| {
        driver.state().turn().number() > 1
    })?;
    assert_eq!(
        controlled(&driver, 0),
        before,
        "each came back at the end step"
    );
    for seat in 1..seats {
        assert!(
            controlled(&driver, seat).is_empty(),
            "seat {seat}'s creature stayed dead"
        );
    }
    let state = replay(driver.engine().setup(), driver.engine().events()).unwrap();
    assert_eq!(state.state_hash(), driver.state().state_hash());
    Ok(())
}
