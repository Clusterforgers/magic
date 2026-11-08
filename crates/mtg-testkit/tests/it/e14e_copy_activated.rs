//! Goal: "Whenever you activate an ability, copy that ability" copies the ability the activation put on the stack, never its source, only for the activations its pattern names; at tables of two to four seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_engine::replay;
use mtg_testkit::cards::forest;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::pid;

/// A creature with one rule.
fn creature(name: &str, rule: &str) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Creature)
            .with_power_toughness(1, 1)
            .with_rules(serde_json::from_str::<Ability>(rule).unwrap()),
    )
}

/// "Whenever you activate an ability, copy that ability."
fn ringer() -> Arc<CardDefinition> {
    creature(
        "Ringer",
        r#"{"Triggered":{"when":{"Did":{"action":"Activated","who":"You"}},"effects":[{"CopyAbility":{"what":"Dealer","new_targets":true}}]}}"#,
    )
}

/// "{0}: You gain 3 life."
fn healer() -> Arc<CardDefinition> {
    creature(
        "Healer",
        r#"{"Activated":{"cost":[],"effects":[{"GainLife":{"amount":3}}]}}"#,
    )
}

/// Answers passively until nothing is on the stack and nothing waits.
fn settle(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..60 {
        if driver.state().stack().is_empty()
            && driver.state().resolving().is_none()
            && driver.state().pending_triggers().is_empty()
        {
            return Ok(());
        }
        driver.answer_passively()?;
    }
    Err(ScriptError::Stuck("the stack".into()))
}

#[rstest]
fn the_activated_ability_is_copied_not_its_source(
    #[values(2, 3, 4)] seats: usize,
    #[values(0, 1)] ringer_seat: usize,
) -> Result<(), ScriptError> {
    let mut mine = vec![ready(healer())];
    let mut theirs = Vec::new();
    if ringer_seat == 0 {
        mine.push(ready(ringer()));
    } else {
        theirs.push(ready(ringer()));
    }
    let scenario = Scenario::new(seats)
        .library(0, vec![forest(); 5])
        .battlefield(0, mine)
        .battlefield(1, theirs);
    let setup = scenario.clone().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario)?;
    let before = driver.state().player(pid(0)).unwrap().life();
    driver.begin_activation("Healer")?;
    settle(&mut driver)?;
    let gained = driver.state().player(pid(0)).unwrap().life() - before;
    // Only the activations of the Ringer's controller are copied; the copy resolves for the Healer's controller.
    let expected = if ringer_seat == 0 { 6 } else { 3 };
    assert_eq!(gained, expected);
    assert!(driver.state().object(driver.id_of("Healer")?).is_some());
    let replayed = replay(&setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
    Ok(())
}
