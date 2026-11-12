//! Goal: the r37-6 corpus cards play at two to four seats: Hama Pashar, Ruin Seeker makes the room abilities of its
//! controller's dungeon trigger twice (two Goblins from the Goblin Lair, one without it), and Aeon Engine reverses the
//! turn order so the next turn goes to the seat on the right.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use common::pid;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{free_creature, w11d_venturer};
use mtg_testkit::corpus::corpus;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// Answers every question passively until the stack and the pending triggers are empty.
fn settle(driver: &mut Driver) -> Result<(), ScriptError> {
    while !driver.state().stack().is_empty() || !driver.state().pending_triggers().is_empty() {
        driver.answer_passively()?;
    }
    Ok(())
}

/// How many Goblins are on the battlefield.
fn goblins(driver: &Driver) -> usize {
    driver
        .state()
        .zone(Zone::Battlefield)
        .iter()
        .filter(|id| {
            driver
                .state()
                .object(**id)
                .is_some_and(|o| o.card().name == "Goblin")
        })
        .count()
}

#[rstest]
fn hama_pashar_makes_each_room_ability_trigger_twice(
    #[values(2, 3, 4)] seats: usize,
    #[values(true, false)] hama: bool,
) -> Result<(), ScriptError> {
    let mut field = vec![ready(free_creature("Bear", 2, 2))];
    if hama {
        field.push(ready(
            corpus().unwrap().card("Hama Pashar, Ruin Seeker").unwrap(),
        ));
    }
    let scenario = Scenario::new(seats)
        .hand(0, [w11d_venturer("Venture 0"), w11d_venturer("Venture 1")])
        .battlefield(0, field);
    let mut driver = Driver::from_scenario(scenario)?;
    for n in 0..2 {
        driver.cast(&format!("Venture {n}"))?;
        settle(&mut driver)?;
    }
    assert_eq!(driver.state().w11d_marker(pid(0)).map(|m| m.room), Some(1));
    assert_eq!(goblins(&driver), if hama { 2 } else { 1 });
    Ok(())
}

#[rstest]
#[case::two_seats(2, 1)]
#[case::three_seats(3, 2)]
#[case::four_seats(4, 3)]
fn aeon_engine_passes_the_next_turn_to_the_right(
    #[case] seats: usize,
    #[case] next: usize,
) -> Result<(), ScriptError> {
    let engine = corpus().unwrap().card("Aeon Engine").unwrap();
    let scenario = Scenario::new(seats).battlefield(0, [ready(engine)]);
    let mut driver = Driver::from_scenario(scenario)?;
    driver.begin_activation("Aeon Engine")?;
    settle(&mut driver)?;
    assert!(driver.state().zone(Zone::Battlefield).iter().all(|id| {
        driver
            .state()
            .object(*id)
            .is_none_or(|o| o.card().name != "Aeon Engine")
    }));
    driver.idle_until(2, Step::Upkeep)?;
    assert_eq!(driver.state().turn().active(), pid(next));
    driver.idle_until(3, Step::Upkeep)?;
    let after = if seats == 2 { 0 } else { next - 1 };
    assert_eq!(driver.state().turn().active(), pid(after));
    Ok(())
}
