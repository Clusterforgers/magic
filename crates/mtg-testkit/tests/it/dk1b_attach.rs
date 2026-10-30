//! Goal: the attach-and-move cards of the owner's deck play as printed at tables of two, three and four seats: Fungal Fortitude returns the creature it enchants tapped when it dies, Unfinished Business returns a creature and attaches the Auras and Equipment it returns, and Danitha puts an Aura or Equipment onto the battlefield attached to herself.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_core::ids::ObjectId;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{burn, free_creature, mountain};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, attached_to, ready};
use rstest::rstest;

use crate::common::copying::replays;
use crate::common::{names, on_battlefield, pid};

/// Passes priority until the stack is empty and nothing is resolving.
fn dk1b_settle(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..200 {
        if driver.state().stack().is_empty() && driver.state().resolving().is_none() {
            return Ok(());
        }
        driver.answer_passively()?;
    }
    Err(ScriptError::Stuck("the stack".into()))
}

/// Whether the battlefield permanent with this name is tapped.
fn dk1b_tapped(driver: &Driver, name: &str) -> bool {
    let id = on_battlefield(driver.state(), name).unwrap();
    driver
        .state()
        .object(id)
        .unwrap()
        .permanent()
        .unwrap()
        .tapped()
}

/// The host of the permanent with this name, by name.
fn dk1b_host_of(driver: &Driver, name: &str) -> Option<String> {
    let id = on_battlefield(driver.state(), name)?;
    let host: ObjectId = driver.state().host_of(id)?;
    Some(driver.state().object(host)?.card().name.clone())
}

#[rstest]
fn dk1b_fungal_fortitude_returns_the_enchanted_creature_tapped(
    #[values(2, 3, 4)] seats: usize,
    #[values(0, 1)] owner: usize,
) -> Result<(), ScriptError> {
    // The enchanted creature belongs to `owner`; the Aura's controller is seat 0 either way.
    let mut worn = vec![
        ready(free_creature("Mine", 2, 2)),
        attached_to(ready(card("Fungal Fortitude").unwrap()), owner, 0),
    ];
    let mut scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .hand(0, [burn("Bolt", 3)]);
    if owner == 0 {
        worn.push(ready(mountain()));
    } else {
        scenario = scenario.battlefield(0, [ready(mountain())]);
    }
    let scenario = scenario.battlefield(owner, worn);
    let setup = scenario.clone().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario)?;
    let target = driver.target_named("Mine")?;
    driver.cast_at("Bolt", &[target])?;
    dk1b_settle(&mut driver)?;
    assert!(dk1b_tapped(&driver, "Mine"));
    assert!(on_battlefield(driver.state(), "Fungal Fortitude").is_none());
    assert!(
        names(driver.state(), Zone::Graveyard(pid(owner))).contains(&"Fungal Fortitude".to_owned())
    );
    replays(&driver, &setup);
    Ok(())
}
