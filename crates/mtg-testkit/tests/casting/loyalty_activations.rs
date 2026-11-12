//! Goal: "Whenever you activate a loyalty ability" (Chandra's Regulator) triggers on a loyalty ability only, never on another activated ability of the same planeswalker, while "Whenever you activate an ability" triggers on both; at tables of two to four seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::pid;

/// A planeswalker with "+1: You gain 1 life." and "{0}: You gain 10 life."
fn walker() -> Arc<CardDefinition> {
    let rules = [
        r#"{"Loyalty":{"cost":{"Plus":1},"effects":[{"GainLife":{"amount":1}}]}}"#,
        r#"{"Activated":{"cost":[],"effects":[{"GainLife":{"amount":10}}]}}"#,
    ];
    let mut card = CardDefinition::new("Walker")
        .with_type(CardType::Planeswalker)
        .with_loyalty(3);
    for rule in rules {
        card = card.with_rules(serde_json::from_str::<Ability>(rule).unwrap());
    }
    Arc::new(card)
}

/// An enchantment with "Whenever you <action> of a planeswalker, you gain 100 life."
fn watcher(action: &str) -> Arc<CardDefinition> {
    let rule = format!(
        r#"{{"Triggered":{{"when":{{"Did":{{"action":"{action}","who":"You","what":[{{"Type":"Planeswalker"}}]}}}},"effects":[{{"GainLife":{{"amount":100}}}}]}}}}"#
    );
    Arc::new(
        CardDefinition::new("Watcher")
            .with_type(CardType::Enchantment)
            .with_rules(serde_json::from_str::<Ability>(&rule).unwrap()),
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
#[case::loyalty_on_a_loyalty_ability("ActivatedLoyalty", 0, 101)]
#[case::loyalty_not_on_another_ability("ActivatedLoyalty", 1, 10)]
#[case::any_on_a_loyalty_ability("Activated", 0, 101)]
#[case::any_on_another_ability("Activated", 1, 110)]
fn a_loyalty_activation_is_its_own_action(
    #[case] action: &str,
    #[case] ability: usize,
    #[case] expected: i32,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(seats).battlefield(0, [ready(walker()), ready(watcher(action))]),
    )?;
    let before = driver.state().player(pid(0)).unwrap().life();
    driver.begin_ability("Walker", ability)?;
    settle(&mut driver)?;
    assert_eq!(
        driver.state().player(pid(0)).unwrap().life() - before,
        expected
    );
    Ok(())
}
