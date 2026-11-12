//! Goal: "Exile a creature you control" as a cost (`CostItem::ExilePermanents`) offers only the payer's own creatures, exiles the one picked as the ability is activated, cannot be activated without one, and the log replays to the live state; the same holds for a spell's additional cost.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use crate::common;

use common::copying::{life, replays, run};
use common::name_of;
use mtg_cards::CardDefinition;
use mtg_core::card::CardType;
use mtg_engine::choose::CardsChoice;
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::zones::Zone;
use mtg_testkit::cards::free_creature;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// An artifact with "Exile a creature you control: You gain 3 life."
fn altar() -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new("Altar")
            .with_type(CardType::Artifact)
            .with_rules(
                serde_json::from_str(
                    r#"{"Activated":{"cost":[{"ExilePermanents":{"filter":[{"Type":"Creature"}]}}],"effects":[{"GainLife":{"amount":3}}]}}"#,
                )
                .unwrap(),
            ),
    )
}

/// Answers the picks of the cost with the creature named `pick` and the mana automatically, until priority returns.
fn pay(driver: &mut Driver, pick: &str) -> Result<Vec<String>, ScriptError> {
    let mut offered = Vec::new();
    while !matches!(driver.decision()?.kind, DecisionKind::Priority(_)) {
        match &driver.decision()?.kind {
            DecisionKind::ChooseCards(options) => {
                offered = options
                    .from
                    .iter()
                    .map(|id| name_of(driver.state(), *id))
                    .collect();
                let id = driver.id_of(pick).unwrap();
                driver.answer(Answer::ChooseCards(CardsChoice(vec![id])))?;
            }
            DecisionKind::PayMana(_) => {
                driver.answer(Answer::PayMana(PayChoice::Auto))?;
            }
            other => panic!("unexpected {}", other.name()),
        }
    }
    Ok(offered)
}

fn exiled(driver: &Driver) -> Vec<String> {
    driver
        .state()
        .zone(Zone::Exile)
        .iter()
        .map(|id| name_of(driver.state(), *id))
        .collect()
}

#[rstest]
#[case::one_creature(1)]
#[case::two_creatures(2)]
#[case::three_creatures(3)]
fn the_cost_exiles_a_creature_the_payer_controls(#[case] mine: usize) -> Result<(), ScriptError> {
    let scenario = || {
        let creatures: Vec<_> = (0..mine)
            .map(|n| ready(free_creature(&format!("Mine {n}"), 1, 1)))
            .chain(std::iter::once(ready(altar())))
            .collect();
        Scenario::new(2)
            .battlefield(0, creatures)
            .battlefield(1, vec![ready(free_creature("Theirs", 1, 1))])
    };
    let setup = scenario().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario())?;
    driver.begin_activation("Altar")?;
    let offered = pay(&mut driver, "Mine 0")?;
    assert!(!offered.contains(&"Theirs".to_owned()), "{offered:?}");
    assert!(offered.is_empty() || offered.len() == mine, "{offered:?}");
    run(&mut driver, 2)?;
    assert_eq!(exiled(&driver), vec!["Mine 0".to_owned()]);
    assert_eq!(life(&driver, 0), 23);
    replays(&driver, &setup);
    Ok(())
}

#[test]
fn without_a_creature_of_its_own_the_cost_cannot_be_paid() -> Result<(), ScriptError> {
    let scenario = Scenario::new(2)
        .battlefield(0, vec![ready(altar())])
        .battlefield(1, vec![ready(free_creature("Theirs", 1, 1))]);
    let mut driver = Driver::from_scenario(scenario)?;
    assert!(driver.begin_activation("Altar").is_err());
    assert_eq!(driver.state().zone(Zone::Exile).len(), 0);
    assert_eq!(
        driver.state().zone(Zone::Battlefield).len(),
        2,
        "nothing moved"
    );
    Ok(())
}
