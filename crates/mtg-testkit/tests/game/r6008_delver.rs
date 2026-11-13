//! Goal: a commander creature under Dungeon Delver makes the room abilities of its owner's dungeon trigger an
//! additional time (a granted `TriggerAgain` over `Pred::Dungeon` in the command zone), and without Delver once, at two
//! to four seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_engine::format::Commander;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{free_creature, w11d_venturer};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// Answers passively until the stack and the pending triggers are empty.
fn r6008_settle(driver: &mut Driver) -> Result<(), ScriptError> {
    while !driver.state().stack().is_empty() || !driver.state().pending_triggers().is_empty() {
        driver.answer_passively()?;
    }
    Ok(())
}

/// The Goblin tokens on the battlefield (Goblin Lair, the second room of Lost Mine of Phandelver, makes one).
fn r6008_goblins(driver: &Driver) -> usize {
    let state = driver.state();
    state
        .zone(Zone::Battlefield)
        .iter()
        .filter(|id| {
            state
                .object(**id)
                .is_some_and(|o| o.card().name == "Goblin")
        })
        .count()
}

#[rstest]
#[case::with_delver(true, 2)]
#[case::without_delver(false, 1)]
fn r6008_room_abilities_trigger_again_under_a_commander_with_delver(
    #[values(2, 3, 4)] seats: usize,
    #[case] delver: bool,
    #[case] goblins: usize,
) -> Result<(), ScriptError> {
    let mut scenario = Scenario::new(seats)
        .format(Arc::new(Commander))
        .commanders(0, [free_creature("Cmd", 2, 2)])
        .hand(0, [w11d_venturer("Venture 0"), w11d_venturer("Venture 1")]);
    if delver {
        scenario = scenario.battlefield(0, [ready(card("Dungeon Delver").unwrap())]);
    }
    for seat in 0..seats {
        scenario = scenario.library(
            seat,
            (0..10).map(|n| free_creature(&format!("Base {n}"), 1, 1)),
        );
    }
    let mut driver = Driver::from_scenario(scenario)?;
    driver.cast("Cmd")?;
    r6008_settle(&mut driver)?;
    driver.cast("Venture 0")?;
    r6008_settle(&mut driver)?;
    driver.cast("Venture 1")?;
    r6008_settle(&mut driver)?;
    assert_eq!(r6008_goblins(&driver), goblins);
    Ok(())
}
