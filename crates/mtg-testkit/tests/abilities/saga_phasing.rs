//! Goal: a Saga that phases out (CR 702.26) keeps its lore counters and is treated as though it does not exist, at tables of two, three and four seats: the last chapter that was on the stack when it phased out cannot exile it, so it stays phased out with its counters and is sacrificed once it phases in (CR 714.4); a Saga that phases out before its last chapter phases in as the same object and returns transformed as a new object without counters when the chapter resolves; a phased-out permanent that its controller moves by hand and back is a new object without counters for the invariant checker too; the log of each replays to the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_cards::value::CounterKind;
use mtg_core::ids::ObjectId;
use mtg_engine::decision::DecisionKind;
use mtg_engine::events::ZoneCause;
use mtg_engine::manual::{ManualAction, ManualZone};
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_engine::{Event, GameSetup, replay};
use mtg_testkit::cards::plains;
use mtg_testkit::saga_cards::transforming_saga;
use mtg_testkit::{Driver, Scenario, ScriptError, ready, with_counters};
use rstest::rstest;

use crate::common::pid;

const TALE: &str = "Tale";
const PROTECTION: &str = "Teferi's Protection";
const PLAINS: [&str; 3] = ["Plains", "Plains", "Plains"];

/// Seat 0 holds Teferi's Protection and controls three Plains and the Saga with `lore` lore counters, at the start of
/// the upkeep of turn 1; Teferi's Protection phases out everything seat 0 controls until its next turn.
fn table(seats: usize, lore: u32) -> Result<(Driver, GameSetup), ScriptError> {
    let scenario = Scenario::new(seats)
        .hand_named(0, [PROTECTION])
        .battlefield(
            0,
            [
                ready(plains()),
                ready(plains()),
                ready(plains()),
                with_counters(ready(transforming_saga(TALE)), CounterKind::lore(), lore),
            ],
        )
        .at(1, 0, Step::Upkeep);
    let setup = scenario.clone().setup().unwrap();
    Ok((Driver::from_scenario(scenario)?, setup))
}

fn zone_of(driver: &Driver, id: ObjectId) -> Zone {
    driver.state().object(id).unwrap().zone()
}

fn lore(driver: &Driver, id: ObjectId) -> u32 {
    driver
        .state()
        .object(id)
        .and_then(|object| object.permanent())
        .map_or(0, |permanent| permanent.counters_of(&CounterKind::lore()))
}

/// The number of the turn on which seat 0 takes its next turn.
fn next_turn(seats: usize) -> u32 {
    u32::try_from(seats).unwrap() + 1
}

/// Puts the triggers waiting to be ordered on the stack in the order listed.
fn put_triggers_on_the_stack(driver: &mut Driver) -> Result<(), ScriptError> {
    let waiting = match &driver.decision()?.kind {
        DecisionKind::OrderTriggers(options) => options.triggers.len(),
        _ => 0,
    };
    if waiting > 0 {
        driver.order_triggers(&(0..waiting).collect::<Vec<_>>())?;
    }
    Ok(())
}

/// Every seat passes until the stack is empty and nothing resolves; the triggers that come are ordered as listed.
fn settle(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..200 {
        put_triggers_on_the_stack(driver)?;
        if driver.state().stack().is_empty() && driver.state().resolving().is_none() {
            return Ok(());
        }
        driver.pass()?;
    }
    Err(ScriptError::Stuck("the stack".into()))
}

/// Every seat passes until `id` is in `zone`.
fn pass_until_in(driver: &mut Driver, id: ObjectId, zone: Zone) -> Result<(), ScriptError> {
    for _ in 0..50 {
        if zone_of(driver, id) == zone {
            return Ok(());
        }
        driver.pass()?;
    }
    Err(ScriptError::Stuck(format!("{id:?} in {zone:?}")))
}

fn replays(driver: &Driver, setup: &GameSetup) {
    let replayed = replay(setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
}

/// The events of the log from `from` on that move `id` between zones.
fn moves_of(driver: &Driver, from: usize, id: ObjectId) -> Vec<Event> {
    driver.engine().events()[from..]
        .iter()
        .filter(|event| matches!(event, Event::ZoneChanged { .. }))
        .filter(|event| event.touched_objects().contains(&id))
        .cloned()
        .collect()
}

#[rstest]
fn the_last_chapter_on_the_stack_cannot_exile_a_saga_that_phased_out(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let (mut driver, setup) = table(seats, 2)?;
    let id = driver.id_of(TALE)?;
    let counter = driver.state().object(id).unwrap().counter();
    // Chapter III triggers as the precombat main phase begins and waits on the stack.
    driver.pass_until(Step::Main1)?;
    put_triggers_on_the_stack(&mut driver)?;
    assert_eq!((driver.state().stack().len(), lore(&driver, id)), (1, 3));
    let from = driver.engine().events().len();
    driver.cast_tapping(PROTECTION, &PLAINS)?;
    pass_until_in(&mut driver, id, Zone::PhasedOut)?;
    assert_eq!(driver.state().stack().len(), 1, "the chapter still waits");
    settle(&mut driver)?;
    // The chapter ability resolved without the Saga: it is no object to exile and to return.
    let object = driver.state().object(id).unwrap();
    assert_eq!(
        (object.zone(), object.face(), object.counter()),
        (Zone::PhasedOut, None, counter)
    );
    assert_eq!(lore(&driver, id), 3);
    assert_eq!(moves_of(&driver, from, id), []);
    // It phases in with its three counters, which are the last chapter's: it is sacrificed (CR 714.4).
    let before = driver.engine().events().len();
    driver.idle_until(next_turn(seats), Step::Upkeep)?;
    let object = driver.state().object(id).unwrap();
    assert_eq!(object.zone(), Zone::Graveyard(pid(0)));
    let log = &driver.engine().events()[before..];
    let phased_in = log
        .iter()
        .position(|e| matches!(e, Event::PermanentPhasedIn { object } if *object == id));
    let sacrificed = log.iter().position(|e| {
        matches!(e, Event::ZoneChanged { object, from: Zone::Battlefield, cause: ZoneCause::Sacrificed, .. } if *object == id)
    });
    assert!(phased_in < sacrificed && sacrificed.is_some(), "{log:?}");
    replays(&driver, &setup);
    Ok(())
}

#[rstest]
fn a_saga_that_phased_out_before_its_last_chapter_returns_transformed_once_it_phases_in(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let (mut driver, setup) = table(seats, 2)?;
    let id = driver.id_of(TALE)?;
    let counter = driver.state().object(id).unwrap().counter();
    driver.cast_tapping(PROTECTION, &PLAINS)?;
    settle(&mut driver)?;
    assert_eq!(zone_of(&driver, id), Zone::PhasedOut);
    // No lore counter comes while it is out.
    assert_eq!(lore(&driver, id), 2);
    driver.idle_until(next_turn(seats), Step::Upkeep)?;
    // The same object with the same counters (CR 702.26: nothing is left or entered).
    let object = driver.state().object(id).unwrap();
    assert_eq!(
        (object.zone(), object.counter()),
        (Zone::Battlefield, counter)
    );
    assert_eq!(lore(&driver, id), 2);
    // The third lore counter makes chapter III trigger as the Saga comes in again.
    let from = driver.engine().events().len();
    driver.idle_until(next_turn(seats), Step::Main1)?;
    settle(&mut driver)?;
    let object = driver.state().object(id).unwrap();
    assert_eq!(
        (object.zone(), object.face(), object.counter()),
        (Zone::Battlefield, Some(1), counter + 2)
    );
    assert_eq!(lore(&driver, id), 0);
    assert!(matches!(
        moves_of(&driver, from, id).as_slice(),
        [
            Event::ZoneChanged {
                from: Zone::Battlefield,
                to: Zone::Exile,
                ..
            },
            Event::ZoneChanged {
                from: Zone::Exile,
                to: Zone::Battlefield,
                ..
            },
        ]
    ));
    replays(&driver, &setup);
    Ok(())
}

#[rstest]
fn a_phased_out_saga_moved_by_hand_and_back_is_a_new_object_without_counters(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let (mut driver, setup) = table(seats, 2)?;
    let id = driver.id_of(TALE)?;
    driver.cast_tapping(PROTECTION, &PLAINS)?;
    settle(&mut driver)?;
    assert_eq!(zone_of(&driver, id), Zone::PhasedOut);
    // The driver checks the invariants after each answer, so the counters of the object that left are not carried over
    // to the one that comes back.
    for to in [ManualZone::Exile, ManualZone::Battlefield] {
        driver.manual(ManualAction::Move { object: id, to })?;
    }
    assert_eq!(zone_of(&driver, id), Zone::Battlefield);
    assert_eq!(lore(&driver, id), 1, "it enters with one lore counter");
    replays(&driver, &setup);
    Ok(())
}
