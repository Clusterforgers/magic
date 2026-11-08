//! Goal: a static ability that gives a keyword to the spells its controller casts works for the keywords that act while the spell is cast, as it does for the keywords that help pay (the engine side of "Spells you cast have cascade").
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_testkit::cards::instant;
use mtg_testkit::{Driver, Scenario, ready};
use rstest::rstest;

fn source(keyword: &str) -> Arc<CardDefinition> {
    let rule: Ability = serde_json::from_str(&format!(
        r#"{{"Static":{{"affected":{{"Each":[{{"ControlledBy":"You"}},{{"WasCast":{{}}}},{{"In":"Stack"}}]}},"mods":[{{"AddKeyword":{keyword}}}]}}}}"#
    ))
    .unwrap();
    Arc::new(
        CardDefinition::new("Source")
            .with_type(CardType::Enchantment)
            .with_rules(rule),
    )
}

/// How many objects the stack holds after a spell is cast under the grant.
fn stack_after_cast(keyword: &str) -> usize {
    let scenario = Scenario::new(2)
        .hand(0, [instant("Bolt", 0, &[])])
        .battlefield(0, [ready(source(keyword))]);
    let mut driver = Driver::from_scenario(scenario).unwrap();
    driver.cast("Bolt").unwrap();
    driver.state().stack().len()
}

#[rstest]
#[case::cascade(r#""Cascade""#)]
#[case::storm(r#""Storm""#)]
#[case::demonstrate(r#""Demonstrate""#)]
#[case::ripple(r#"{"Ripple":4}"#)]
fn a_granted_cast_trigger_goes_on_the_stack_with_the_spell(#[case] keyword: &str) {
    assert!(stack_after_cast(keyword) >= 2, "{keyword}");
}
