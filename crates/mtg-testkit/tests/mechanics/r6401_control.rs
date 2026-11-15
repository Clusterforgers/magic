//! Goal: Risky Move plays as printed at two to four seats: each upkeep its active player gains control of it, and the
//! new controller then flips a coin, giving a creature of theirs to the opponent they chose only when they lose; a
//! player who already controls it gains nothing and flips no coin.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_engine::Event;
use mtg_engine::characteristics::controller_of;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{bear, free_creature};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::{name_of, pid};

/// The controller of the only permanent of this name.
fn r6401_controller(driver: &Driver, name: &str) -> usize {
    let state = driver.state();
    let id = state
        .zone(Zone::Battlefield)
        .iter()
        .copied()
        .find(|id| name_of(state, *id) == name)
        .unwrap();
    let controller = controller_of(state, id).unwrap();
    (0..4).find(|seat| pid(*seat) == controller).unwrap()
}

/// The coin flips of the game so far, won or lost.
fn r6401_flips(driver: &Driver) -> Vec<bool> {
    driver
        .engine()
        .events()
        .iter()
        .filter_map(|event| match event {
            Event::CoinFlipped { won, .. } => Some(*won),
            _ => None,
        })
        .collect()
}

#[rstest]
fn r6401_risky_move_changes_hands_and_risks_a_creature(
    #[values(2, 3, 4)] seats: usize,
    #[values(1, 2, 3, 4)] seed: u64,
) -> Result<(), ScriptError> {
    let mut scenario = Scenario::new(seats)
        .seed(seed)
        .at(1, 0, Step::Main1)
        .battlefield(0, [ready(card("Risky Move").unwrap())]);
    scenario = scenario.battlefield(1, [ready(bear())]);
    for seat in 2..seats {
        scenario = scenario.battlefield(seat, [ready(free_creature("Ogre", 3, 3))]);
    }
    let mut driver = Driver::from_scenario(scenario)?;
    // Seat 0 controls it already: its own upkeep would change nothing, and no coin was flipped.
    assert!(r6401_flips(&driver).is_empty());
    driver.idle_until(2, Step::Main1)?;
    assert_eq!(r6401_controller(&driver, "Risky Move"), 1);
    let flips = r6401_flips(&driver);
    assert_eq!(flips.len(), 1, "one flip for one change of control");
    let bears = r6401_controller(&driver, "Grizzly Bears");
    if flips[0] {
        assert_eq!(bears, 1, "a won flip keeps the creature");
    } else {
        assert_ne!(bears, 1, "a lost flip gives the creature to an opponent");
    }
    Ok(())
}
