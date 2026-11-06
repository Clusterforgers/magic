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
    let mut driver = Driver::from_scenario(Scenario::new(2).battlefield(0, battlefield).at(
        1,
        0,
        Step::Main1,
    ))?;
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

const BREAK: &str = r#"{"Activated":{"cost":[{"Sacrifice":{"filter":[{"Is":"Granter"}]}}],"effects":[{"GainLife":{"amount":3}}]}}"#;

/// An Equipment: "Equipped creature has 'Sacrifice this Equipment: You gain 3 life.'"
fn breakable_stick() -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new("Breakable Stick")
            .with_type(CardType::Artifact)
            .with_subtype("Equipment")
            .with_mana_cost(cost(1, &[]))
            .with_rules(rules(&format!(
                r#"{{"Static":{{"affected":"Attached","mods":[{{"GrantAbility":{BREAK}}}]}}}}"#
            ))),
    )
}

#[test]
fn a_cost_that_sacrifices_the_granter_takes_the_equipment_and_not_the_creature()
-> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(2)
            .battlefield(
                0,
                vec![ready(bear()), attached_to(ready(breakable_stick()), 0, 0)],
            )
            .at(1, 0, Step::Main1),
    )?;
    let life = driver.state().player(crate::common::pid(0)).unwrap().life();
    driver.begin_activation("Grizzly Bears")?;
    for _ in 0..20 {
        if driver.state().stack().is_empty() && driver.state().resolving().is_none() {
            break;
        }
        driver.answer_passively()?;
    }
    assert!(
        driver.id_of("Breakable Stick").is_err(),
        "the Equipment was sacrificed"
    );
    assert!(driver.id_of("Grizzly Bears").is_ok(), "the creature stays");
    assert_eq!(
        driver.state().player(crate::common::pid(0)).unwrap().life(),
        life + 3
    );
    Ok(())
}

const MAY_BREAK: &str = r#"{"Activated":{"cost":["Tap"],"effects":[{"Pay":{"cost":[{"Sacrifice":{"filter":[{"Is":"Granter"}]}}],"paid":[{"GainLife":{"amount":3}}],"unpaid":[]}}]}}"#;

/// An Equipment: "Equipped creature has '{T}: You may sacrifice this Equipment. If you do, you gain 3 life.'"
fn pay_stick() -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new("Pay Stick")
            .with_type(CardType::Artifact)
            .with_subtype("Equipment")
            .with_mana_cost(cost(1, &[]))
            .with_rules(rules(&format!(
                r#"{{"Static":{{"affected":"Attached","mods":[{{"GrantAbility":{MAY_BREAK}}}]}}}}"#
            ))),
    )
}

#[rstest]
#[case::yes(true)]
#[case::no(false)]
fn paying_with_the_granter_in_the_middle_of_a_resolution_sacrifices_the_equipment_only_on_yes(
    #[case] yes: bool,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(2)
            .battlefield(
                0,
                vec![ready(bear()), attached_to(ready(pay_stick()), 0, 0)],
            )
            .at(1, 0, Step::Main1),
    )?;
    let life = driver.state().player(crate::common::pid(0)).unwrap().life();
    driver.begin_activation("Grizzly Bears")?;
    for _ in 0..20 {
        if driver.state().stack().is_empty() && driver.state().resolving().is_none() {
            break;
        }
        if matches!(
            driver.decision()?.kind,
            mtg_engine::decision::DecisionKind::YesNo(_)
        ) {
            driver.answer(mtg_engine::decision::Answer::YesNo(
                mtg_engine::choose::YesNoChoice(yes),
            ))?;
        } else {
            driver.answer_passively()?;
        }
    }
    assert_eq!(driver.id_of("Pay Stick").is_err(), yes);
    assert!(driver.id_of("Grizzly Bears").is_ok());
    assert_eq!(
        driver.state().player(crate::common::pid(0)).unwrap().life(),
        life + if yes { 3 } else { 0 }
    );
    Ok(())
}
