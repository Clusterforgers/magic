//! Goal: a forecast that sets up a trigger on its target for the turn ("Whenever target creature deals damage this turn,
//! you gain that much life.") is activated from the hand in its owner's upkeep, gains its controller the damage the target
//! deals and nothing for another creature's damage, at tables of two to four seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common::pid;
use mtg_engine::turn::Step;
use mtg_testkit::cards::{bear, plains};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// Seat 0's upkeep: it holds Paladin of Prahv and controls two Plains, Grizzly Bears and Craw Wurm.
fn table(seats: usize) -> Scenario {
    Scenario::new(seats)
        .at(2, 0, Step::Upkeep)
        .hand(0, vec![card("Paladin of Prahv").unwrap()])
        .battlefield(
            0,
            vec![
                ready(plains()),
                ready(plains()),
                ready(bear()),
                ready(mtg_testkit::cards::giant()),
            ],
        )
}

fn life(driver: &Driver, seat: usize) -> i32 {
    driver.state().player(pid(seat)).unwrap().life()
}

#[rstest]
#[case::the_target_alone_attacks(&["Grizzly Bears"], 2)]
#[case::both_attack(&["Grizzly Bears", "Craw Wurm"], 2)]
fn the_targets_damage_is_gained(
    #[case] attackers: &[&str],
    #[case] gained: i32,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table(seats))?;
    driver.begin_activation("Paladin of Prahv")?;
    let bears = driver.target_named("Grizzly Bears")?;
    driver.choose_targets(&[bears])?;
    driver.pay_tapping(&["Plains", "Plains"], 0)?;
    driver.pass_until(Step::DeclareAttackers)?;
    let attacks: Vec<(&str, usize)> = attackers.iter().map(|name| (*name, 1)).collect();
    driver.attack(&attacks)?;
    driver.idle_until(2, Step::Main2)?;
    while !driver.state().stack().is_empty() {
        driver.pass()?;
    }
    assert_eq!(life(&driver, 0), 20 + gained);
    Ok(())
}
