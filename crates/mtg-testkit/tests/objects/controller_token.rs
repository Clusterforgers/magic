//! Goal: the controller of a permanent that was destroyed, and not the caster, creates the token of "Destroy target permanent. Its controller creates a 3/3 green Beast creature token." at tables of two to four seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use common::removal::{owner_of, resolve_the_stack, table};
use common::{names, on_battlefield, pid};
use mtg_engine::zones::Zone;
use mtg_testkit::cards::forest;
use mtg_testkit::{Driver, ScriptError};
use rstest::rstest;

#[rstest]
fn the_controller_of_the_destroyed_permanent_gets_the_beast(
    #[values((2, "Grizzly Bears"), (3, "Serra Angel"), (4, "Llanowar Elves"))] case: (usize, &str),
) -> Result<(), ScriptError> {
    let (seats, victim) = case;
    let mut driver = Driver::from_scenario(table(
        seats,
        &["Beast Within"],
        &[forest(), forest(), forest()],
        &[],
    ))?;
    let owner = owner_of(&driver, victim);
    assert_ne!(owner, 0, "the caster is seat 0");
    let target = driver.target_named(victim)?;
    driver.cast_at("Beast Within", &[target])?;
    resolve_the_stack(&mut driver)?;

    assert!(on_battlefield(driver.state(), victim).is_none());
    assert!(names(driver.state(), Zone::Graveyard(pid(owner))).contains(&victim.to_owned()));
    let beast = on_battlefield(driver.state(), "Beast").expect("the Beast token");
    let token = driver.state().object(beast).unwrap();
    assert_eq!(token.controller().index(), owner);
    Ok(())
}
