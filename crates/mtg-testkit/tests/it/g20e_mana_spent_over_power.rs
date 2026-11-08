//! Goal: "whenever you cast a spell, if the mana spent to cast it is greater than this creature's power" counts the mana
//! paid for that spell and compares it with the power of the creature with the ability.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::value::CounterKind;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{bear, elf, forest};
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::name_of;

/// A 1/1 that grows when a spell costs more mana than its power, as the translator writes Liberator's trigger.
fn grower() -> Arc<CardDefinition> {
    let rules = r#"{"Triggered":{"when":{"SpellCast":{"by":"You","what":["Spell",{"Test":{"left":{"Stat":{"of":"Candidate","stat":"ManaSpent"}},"op":"Gt","right":{"Stat":{"of":"This","stat":"Power"}}}}]}},"effects":[{"PutCounters":{"on":"This","counter":"+1/+1"}}]}}"#;
    Arc::new(
        CardDefinition::new("Grower")
            .with_type(mtg_core::card::CardType::Creature)
            .with_power_toughness(1, 1)
            .with_rules(serde_json::from_str(rules).unwrap()),
    )
}

#[rstest]
#[case::two_mana_over_power_one(bear(), 1)]
#[case::one_mana_is_not_more(elf(), 0)]
fn counts_the_mana_paid(
    #[case] spell: Arc<CardDefinition>,
    #[case] counters: u32,
) -> Result<(), ScriptError> {
    let name = spell.name.clone();
    let scenario = Scenario::new(2)
        .battlefield(0, [ready(grower()), ready(forest()), ready(forest())])
        .hand(0, [spell]);
    let mut driver = Driver::from_scenario(scenario)?;
    driver.cast(&name)?;
    while !driver.state().stack().is_empty() {
        driver.pass()?;
    }
    let state = driver.state();
    let held = state
        .zone(Zone::Battlefield)
        .iter()
        .find(|id| name_of(state, **id) == "Grower")
        .and_then(|id| state.object(*id))
        .and_then(|object| object.permanent())
        .map_or(0, |permanent| permanent.counters_of(&CounterKind::PlusOnePlusOne));
    assert_eq!(held, counters);
    Ok(())
}
