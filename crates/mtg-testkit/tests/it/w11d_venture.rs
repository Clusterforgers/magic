//! Goal: venturing into the dungeon (CR 701.49, 309) works at tables of two, three and four seats: the first venture puts Lost Mine of Phandelver in the player's command zone at its first room, each later one moves the marker down an arrow and triggers that room's ability, the last room's ability resolves before the dungeon is completed and leaves the game, the next venture enters a new dungeon, the other seats have no dungeon, and the log replays to the state it was played to.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use common::pid;
use mtg_engine::zones::Zone;
use mtg_engine::{Event, replay};
use mtg_testkit::cards::{free_creature, w11d_venturer};
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

const VENTURES: usize = 5;

/// Seat 0 holds five Ventures and a Bear; every seat has a library to draw from.
fn table(seats: usize) -> Scenario {
    let hand: Vec<_> = (0..VENTURES)
        .map(|n| w11d_venturer(&format!("Venture {n}")))
        .collect();
    Scenario::new(seats)
        .hand(0, hand)
        .battlefield(0, [ready(free_creature("Bear", 2, 2))])
}

/// Casts the Venture and answers every question passively until the stack is empty.
fn venture(driver: &mut Driver, n: usize) -> Result<(), ScriptError> {
    driver.cast(&format!("Venture {n}"))?;
    while !driver.state().stack().is_empty() || !driver.state().pending_triggers().is_empty() {
        driver.answer_passively()?;
    }
    Ok(())
}

fn rooms(driver: &Driver) -> Vec<String> {
    driver
        .engine()
        .events()
        .iter()
        .filter_map(|event| match event {
            Event::Ventured(ventured) => Some(ventured.room_name.clone()),
            _ => None,
        })
        .collect()
}

fn dungeons_in_command(driver: &Driver, seat: usize) -> usize {
    driver.state().zone(Zone::Command(pid(seat))).len()
}

#[rstest]
fn a_player_ventures_through_lost_mine_of_phandelver_and_completes_it(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = table(seats);
    let setup = scenario.clone().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario)?;
    venture(&mut driver, 0)?;
    assert_eq!(dungeons_in_command(&driver, 0), 1);
    assert_eq!(driver.state().w11d_marker(pid(0)).map(|m| m.room), Some(0));
    for n in 1..4 {
        venture(&mut driver, n)?;
    }
    assert_eq!(
        rooms(&driver),
        [
            "Cave Entrance",
            "Goblin Lair",
            "Storeroom",
            "Temple of Dumathoin"
        ]
    );
    let goblins = driver
        .state()
        .zone(Zone::Battlefield)
        .iter()
        .filter(|id| {
            driver
                .state()
                .object(**id)
                .is_some_and(|o| o.card().name == "Goblin")
        })
        .count();
    assert_eq!(goblins, 1, "the Goblin Lair made a Goblin");
    let completed: Vec<_> = driver
        .engine()
        .events()
        .iter()
        .filter(|event| matches!(event, Event::DungeonCompleted { .. }))
        .collect();
    assert_eq!(completed.len(), 1);
    assert_eq!(driver.state().w11d_completed(pid(0)), 1);
    assert_eq!(driver.state().w11d_marker(pid(0)), None);
    assert_eq!(dungeons_in_command(&driver, 0), 0);
    venture(&mut driver, 4)?;
    assert_eq!(driver.state().w11d_marker(pid(0)).map(|m| m.room), Some(0));
    for seat in 1..seats {
        assert_eq!(dungeons_in_command(&driver, seat), 0);
        assert_eq!(driver.state().w11d_completed(pid(seat)), 0);
    }
    let replayed = replay(&setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.engine().state_hash());
    Ok(())
}
