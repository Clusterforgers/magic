//! Goal: a permanent that gains the abilities of other cards gains the mana ability of a basic land (CR 305.6) like any printed mana ability.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_core::mana::ManaType;
use mtg_testkit::cards::{forest, plains};
use mtg_testkit::{Driver, Scenario, ScriptError, ready};

fn borrower() -> Arc<CardDefinition> {
    let rules: Ability = serde_json::from_str(
        r#"{"Static":{"affected":"This","mods":[{"GainAbilitiesOf":{"Each":[{"Type":"Land"},{"In":"Graveyard"}]}}]}}"#,
    )
    .unwrap();
    Arc::new(
        CardDefinition::new("Borrower")
            .with_type(CardType::Creature)
            .with_power_toughness(1, 1)
            .with_rules(rules),
    )
}

#[test]
fn a_permanent_gains_the_mana_ability_of_a_basic_land_in_a_graveyard() -> Result<(), ScriptError> {
    let scenario = Scenario::new(2)
        .battlefield(0, [ready(borrower())])
        .graveyard(0, [forest()]);
    let mut driver = Driver::from_scenario(scenario)?;
    driver.tap_for_mana_of("Borrower", ManaType::Green)?;
    Ok(())
}

#[test]
fn each_basic_land_lends_its_own_color() -> Result<(), ScriptError> {
    let scenario = Scenario::new(2)
        .battlefield(0, [ready(borrower())])
        .graveyard(0, [plains()]);
    let mut driver = Driver::from_scenario(scenario)?;
    driver.tap_for_mana_of("Borrower", ManaType::White)?;
    Ok(())
}
