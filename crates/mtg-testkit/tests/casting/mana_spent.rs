//! Goal: "sacrifice it unless {M} was spent to cast it" reads the mana the cast spent at tables of two, three and four seats: the creature stays when one mana of that type paid for it, goes to the graveyard when none did (a cast paid with other colors, or any mana at all, spends none of it), and a creature put onto the battlefield without being cast spent no mana, so it is sacrificed.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_engine::decision::DecisionKind;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{island, mountain, plains};
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::pid;

/// Seat 0 holds the creature named and the lands given; the game stands in seat 0's first main phase.
fn table(
    seats: usize,
    creature: &str,
    lands: &[fn() -> std::sync::Arc<mtg_cards::CardDefinition>],
) -> Result<Driver, ScriptError> {
    let scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .hand_named(0, [creature])
        .battlefield(0, lands.iter().map(|land| ready(land())));
    Driver::from_scenario(scenario)
}

/// Passes and orders until the stack is empty again.
fn settle(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..40 {
        let ordering = if let DecisionKind::OrderTriggers(options) = &driver.decision()?.kind {
            Some((0..options.triggers.len()).collect::<Vec<usize>>())
        } else {
            None
        };
        if let Some(order) = ordering {
            driver.order_triggers(&order)?;
        } else if driver.state().stack().is_empty() && driver.state().resolving().is_none() {
            return Ok(());
        } else {
            driver.pass()?;
        }
    }
    Err(ScriptError::Stuck("the stack".into()))
}

/// Where the creature of seat 0 is after it was cast with the lands tapped.
fn cast_with(
    seats: usize,
    creature: &str,
    lands: &[fn() -> std::sync::Arc<mtg_cards::CardDefinition>],
    tapped: &[&str],
) -> Result<Zone, ScriptError> {
    let mut driver = table(seats, creature, lands)?;
    driver.cast_tapping(creature, tapped)?;
    settle(&mut driver)?;
    let state = driver.state();
    let card = state
        .objects()
        .find(|object| object.card().name == creature)
        .unwrap();
    Ok(card.zone())
}

#[rstest]
#[case::two(2)]
#[case::three(3)]
#[case::four(4)]
fn a_creature_stays_when_the_mana_it_asks_for_paid_and_goes_when_it_did_not(#[case] seats: usize) {
    // Azorius Herald costs {2}{W} and wants {U}: an Island that pays part of the cost spends blue mana.
    let herald = "Azorius Herald";
    let blue = cast_with(
        seats,
        herald,
        &[plains, plains, island],
        &["Plains", "Plains", "Island"],
    )
    .unwrap();
    assert_eq!(blue, Zone::Battlefield);
    let white = cast_with(
        seats,
        herald,
        &[plains, plains, plains],
        &["Plains", "Plains", "Plains"],
    )
    .unwrap();
    assert_eq!(white, Zone::Graveyard(pid(0)));
    let red = cast_with(
        seats,
        herald,
        &[plains, plains, mountain],
        &["Plains", "Plains", "Mountain"],
    )
    .unwrap();
    assert_eq!(red, Zone::Graveyard(pid(0)));
    let unused = cast_with(
        seats,
        herald,
        &[plains, plains, plains, island],
        &["Plains", "Plains", "Plains"],
    )
    .unwrap();
    assert_eq!(
        unused,
        Zone::Graveyard(pid(0)),
        "an Island that stays untapped spends nothing"
    );
}

#[rstest]
#[case::two(2)]
#[case::three(3)]
#[case::four(4)]
fn the_creature_that_stays_keeps_what_its_other_trigger_did(#[case] seats: usize) {
    let mut driver = table(seats, "Azorius Herald", &[plains, plains, island]).unwrap();
    let life = driver.state().player(pid(0)).unwrap().life();
    driver
        .cast_tapping("Azorius Herald", &["Plains", "Plains", "Island"])
        .unwrap();
    settle(&mut driver).unwrap();
    assert_eq!(driver.state().player(pid(0)).unwrap().life(), life + 4);
    let id = driver.id_of("Azorius Herald").unwrap();
    assert_eq!(
        driver.state().object(id).unwrap().mana_spent().len(),
        3,
        "the permanent remembers the three mana of its cast"
    );
}
