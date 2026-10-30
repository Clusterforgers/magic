//! Goal: a seat flips a coin or rolls a die by hand: the engine draws the result from the game's random stream (one position per roll, the same seed the same result, every face over many seeds), logs it as one public event after the action, takes it at any time from any seat, refuses a die outside 2 to 100 faces by name with the game untouched, and a rebuilt engine reaches the same state and stream position.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeSet;

use mtg_core::ids::PlayerId;
use mtg_engine::error::IllegalAnswer;
use mtg_engine::manual::{ManualAction, ManualRefusal, ManualRoll, check};
use mtg_engine::zones::Zone;
use mtg_engine::{Engine, EngineError, Event};
use mtg_testkit::cards::{forest, free_creature};
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::pid;

fn game(seed: u64) -> Driver {
    Driver::from_scenario(
        Scenario::new(2)
            .seed(seed)
            .hand(0, [free_creature("A", 1, 1)])
            .library(0, vec![forest(); 4])
            .battlefield(0, [ready(free_creature("Elf", 1, 1))])
            .library(1, vec![forest(); 4]),
    )
    .unwrap()
}

fn die(sides: u32) -> ManualAction {
    ManualAction::Roll {
        kind: ManualRoll::Die { sides },
    }
}

fn coin() -> ManualAction {
    ManualAction::Roll {
        kind: ManualRoll::Coin {},
    }
}

/// The kind and the result of the roll in `events`, which hold the action and then the roll.
fn rolled(events: &[Event]) -> (ManualRoll, u32) {
    let [
        Event::ManualAction { .. },
        Event::ManualRolled { kind, result, .. },
    ] = events
    else {
        panic!("not an action and a roll: {events:?}");
    };
    (*kind, *result)
}

#[rstest]
#[case(coin(), 2)]
#[case(die(2), 2)]
#[case(die(6), 6)]
#[case(die(20), 20)]
#[case(die(100), 100)]
fn a_roll_is_a_face_of_the_die_and_the_faces_come_up(
    #[case] action: ManualAction,
    #[case] sides: u32,
) {
    let mut seen = BTreeSet::new();
    for seed in 0..400 {
        let mut driver = game(seed);
        let (_, result) = rolled(&driver.manual(action.clone()).unwrap());
        assert!((1..=sides).contains(&result), "seed {seed}: {result}");
        seen.insert(result);
    }
    // 400 draws of a fair die of up to 20 faces leave none out; of 100 faces most come up.
    let wanted = if sides <= 20 { sides as usize } else { 60 };
    assert!(seen.len() >= wanted, "{sides} faces: {seen:?}");
}

#[rstest]
fn the_event_names_who_rolled_and_the_stream_moves_on_by_one_for_each_roll(
    #[values(0, 1)] seat: usize,
) {
    let mut driver = game(7);
    let before = driver.state().rng_index();
    let events = driver.manual_by(pid(seat), &die(20)).unwrap();
    let (kind, result) = rolled(&events);
    assert_eq!(kind, ManualRoll::Die { sides: 20 });
    assert!(matches!(
        &events[1],
        Event::ManualRolled { player, .. } if *player == pid(seat)
    ));
    assert_eq!(driver.state().rng_index(), before + 1);
    let second = driver.manual_by(pid(seat), &die(20)).unwrap();
    assert_eq!(driver.state().rng_index(), before + 2);
    assert!((1..=20).contains(&result) && (1..=20).contains(&rolled(&second).1));
}

#[rstest]
fn the_same_seed_gives_the_same_roll_and_a_rebuilt_engine_the_same_state(
    #[values(coin(), die(6), die(100))] action: ManualAction,
) {
    let (mut first, mut second) = (game(11), game(11));
    let one = rolled(&first.manual(action.clone()).unwrap());
    assert_eq!(rolled(&second.manual(action).unwrap()), one);
    let rebuilt = Engine::resume(
        first.engine().setup().clone(),
        first.engine().events().to_vec(),
    )
    .unwrap();
    assert_eq!(rebuilt.state_hash(), first.state().state_hash());
    assert_eq!(rebuilt.state().rng_index(), first.state().rng_index());
}

#[rstest]
fn a_roll_changes_nothing_but_the_stream(#[values(coin(), die(20))] action: ManualAction) {
    let mut driver = game(3);
    let state = driver.state().clone();
    driver.manual(action).unwrap();
    let after = driver.state();
    assert_eq!(after.rng_index(), state.rng_index() + 1);
    for seat in [PlayerId(0), PlayerId(1)] {
        for zone in [Zone::Hand(seat), Zone::Library(seat), Zone::Graveyard(seat)] {
            assert_eq!(after.zone(zone), state.zone(zone), "{zone:?}");
        }
        assert_eq!(
            after.player(seat).unwrap().life(),
            state.player(seat).unwrap().life()
        );
    }
    assert_eq!(after.zone(Zone::Battlefield), state.zone(Zone::Battlefield));
}

#[rstest]
#[case(0)]
#[case(1)]
#[case(101)]
#[case(u32::MAX)]
fn a_die_outside_two_to_a_hundred_faces_is_refused_and_nothing_changes(#[case] sides: u32) {
    let mut driver = game(5);
    let hash = driver.state().state_hash();
    let log = driver.engine().events().len();
    assert_eq!(
        check(driver.state(), pid(0), &die(sides)),
        Err(ManualRefusal::OutOfRange("the number of faces"))
    );
    let error = driver.manual(die(sides)).unwrap_err();
    assert!(matches!(
        error,
        ScriptError::Engine(EngineError::Illegal(IllegalAnswer::Manual(
            ManualRefusal::OutOfRange("the number of faces")
        )))
    ));
    assert_eq!(driver.state().state_hash(), hash);
    assert_eq!(driver.engine().events().len(), log);
}

#[rstest]
fn the_limits_of_the_range_are_allowed(#[values(2, 100)] sides: u32) {
    let driver = game(5);
    assert_eq!(check(driver.state(), pid(1), &die(sides)), Ok(()));
}
