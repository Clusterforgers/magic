//! Goal: Devouring Tendrils has the creature its controller targets deal damage equal to its power to the permanent of an opponent it targets, and its controller gains 2 life when that permanent dies this turn and not when it survives, at tables of two, three and four seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use common::copying::life;
use mtg_engine::Answer;
use mtg_engine::decision::PayChoice;
use mtg_testkit::cards::{forest, free_creature};
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

#[rstest]
#[case::the_foe_dies(2, 22)]
#[case::the_foe_survives(5, 20)]
fn tendrils_gain_life_when_the_other_permanent_dies(
    #[case] toughness: i32,
    #[case] gained: i32,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(seats)
            .hand_named(0, ["Devouring Tendrils"])
            .battlefield(
                0,
                [
                    ready(free_creature("Mine", 3, 3)),
                    ready(forest()),
                    ready(forest()),
                ],
            )
            .battlefield(1, [ready(free_creature("Foe", 1, toughness))]),
    )?;
    let (mine, foe) = (driver.target_named("Mine")?, driver.target_named("Foe")?);
    driver.begin_cast("Devouring Tendrils")?;
    driver.choose_targets(&[mine])?;
    driver.choose_targets(&[foe])?;
    driver.answer(Answer::PayMana(PayChoice::Auto))?;
    while !driver.state().stack().is_empty() {
        driver.pass()?;
    }
    assert_eq!(life(&driver, 0), gained);
    Ok(())
}
