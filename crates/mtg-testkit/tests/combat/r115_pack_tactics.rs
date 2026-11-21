//! Goal: pack tactics at 2 to 4 seats (CR 508.1, 603.4): "if you attacked with creatures with total power 6 or greater
//! this combat" adds the power of each creature its controller declared as an attacker, as it was declared, so the
//! trigger does its work at 6 and more and nothing below; an opponent's attackers never count, and the log replays to
//! the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_engine::replay;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{free_creature, plains};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::pid;

/// Plays turn 2 of `seats` seats to its end of combat: seat 0 attacks seat 1 with Werewolf Pack Leader (3/3) and a
/// creature of power `power`; every other seat has a 7/7 that does not attack.
fn attack_with_leader(seats: usize, power: i32) -> Result<Driver, ScriptError> {
    let mut scenario = Scenario::new(seats)
        .at(2, 0, Step::DeclareAttackers)
        .battlefield(
            0,
            vec![
                ready(card("Werewolf Pack Leader").unwrap()),
                ready(free_creature("Packmate", power, 9)),
            ],
        )
        .library(0, [plains(), plains()]);
    for seat in 1..seats {
        scenario = scenario.battlefield(
            seat,
            vec![ready(free_creature(&format!("Giant {seat}"), 7, 7))],
        );
    }
    let mut driver = Driver::from_scenario(scenario)?;
    driver.attack(&[("Werewolf Pack Leader", 1), ("Packmate", 1)])?;
    driver.idle_until(2, Step::EndCombat)?;
    Ok(driver)
}

#[rstest]
fn r115_pack_tactics_draws_at_total_power_six(
    #[values(2, 3, 4)] seats: usize,
    #[values((0, false), (2, false), (3, true), (4, true))] case: (i32, bool),
) -> Result<(), ScriptError> {
    let (power, draws) = case;
    let driver = attack_with_leader(seats, power)?;
    assert_eq!(
        driver.state().zone(Zone::Hand(pid(0))).len(),
        usize::from(draws),
        "packmate power {power}"
    );
    let state = replay(driver.engine().setup(), driver.engine().events()).unwrap();
    assert_eq!(state.state_hash(), driver.state().state_hash());
    Ok(())
}

#[rstest]
fn r115_pack_tactics_counts_only_its_controllers_attackers(
    #[values(2, 3, 4)] seats: usize,
    #[values((3, 0), (4, 1))] case: (i32, u32),
) -> Result<(), ScriptError> {
    let (power, put) = case;
    let mut scenario = Scenario::new(seats)
        .at(2, 0, Step::DeclareAttackers)
        .battlefield(
            0,
            vec![
                ready(card("Gnoll Hunter").unwrap()),
                ready(free_creature("Packmate", power, 9)),
            ],
        );
    for seat in 1..seats {
        scenario = scenario.battlefield(
            seat,
            vec![ready(free_creature(&format!("Giant {seat}"), 7, 7))],
        );
    }
    let mut driver = Driver::from_scenario(scenario)?;
    driver.attack(&[("Gnoll Hunter", 1), ("Packmate", 1)])?;
    driver.idle_until(2, Step::EndCombat)?;
    let hunter = driver.id_of("Gnoll Hunter")?;
    let counters = driver
        .state()
        .object(hunter)
        .unwrap()
        .permanent()
        .unwrap()
        .counters()
        .map(|(_, n)| n)
        .sum::<u32>();
    assert_eq!(counters, put, "packmate power {power}");
    Ok(())
}
