//! Goal: the permanent that gave a creature an ability is what the words of that ability call "this Equipment" (`Ref::Granter`): the counter goes on the Equipment and not the creature, and an ability the creature has of its own names nobody.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_core::ids::ObjectId;
use mtg_engine::turn::Step;
use mtg_testkit::cards::{bear, cost};
use mtg_testkit::{Driver, Scenario, ScriptError, attached_to, ready};
use rstest::rstest;

fn rules(json: &str) -> Ability {
    serde_json::from_str(json).unwrap()
}

const TICK: &str = r#"{"Activated":{"cost":["Tap"],"effects":[{"PutCounters":{"on":"Granter","counter":"+1/+1"}}]}}"#;

/// An Equipment: "Equipped creature has '{T}: Put a +1/+1 counter on this Equipment.'"
fn tally_stick() -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new("Tally Stick")
            .with_type(CardType::Artifact)
            .with_subtype("Equipment")
            .with_mana_cost(cost(1, &[]))
            .with_rules(rules(&format!(
                r#"{{"Static":{{"affected":"Attached","mods":[{{"GrantAbility":{TICK}}}]}}}}"#
            ))),
    )
}

/// A creature with the same words printed on it.
fn born_ticker() -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new("Born Ticker")
            .with_type(CardType::Creature)
            .with_mana_cost(cost(1, &[]))
            .with_power_toughness(1, 1)
            .with_rules(rules(TICK)),
    )
}

fn counters(driver: &Driver, id: ObjectId) -> u32 {
    driver
        .state()
        .object(id)
        .unwrap()
        .permanent()
        .unwrap()
        .counters()
        .map(|(_, count)| count)
        .sum()
}

#[rstest]
#[case::granted(true)]
#[case::printed(false)]
fn the_counter_goes_on_the_equipment_that_gave_the_ability(
    #[case] equipped: bool,
) -> Result<(), ScriptError> {
    let (host, battlefield) = if equipped {
        (
            "Grizzly Bears",
            vec![ready(bear()), attached_to(ready(tally_stick()), 0, 0)],
        )
    } else {
        ("Born Ticker", vec![ready(born_ticker())])
    };
    let mut driver = Driver::from_scenario(
        Scenario::new(2)
            .battlefield(0, battlefield)
            .at(1, 0, Step::Main1),
    )?;
    driver.begin_activation(host)?;
    for _ in 0..20 {
        if driver.state().stack().is_empty() && driver.state().resolving().is_none() {
            break;
        }
        driver.answer_passively()?;
    }
    let host_id = driver.id_of(host)?;
    assert_eq!(counters(&driver, host_id), 0);
    if equipped {
        assert_eq!(counters(&driver, driver.id_of("Tally Stick")?), 1);
    }
    Ok(())
}
