//! Goal: a loyalty ability a static ability gives ("Planeswalkers you control have '[0]: You gain 10 life.'") is offered beside the printed one, resolves, changes loyalty by its cost and shares the one loyalty activation of the turn; without the giver it is not offered; at tables of two to four seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::value::CounterKind;
use mtg_core::card::CardType;
use mtg_engine::abilities::AbilitySource;
use mtg_engine::decision::{Answer, DecisionKind, PriorityAction};
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::pid;

/// A planeswalker with "+1: You gain 1 life." and loyalty 3.
fn walker() -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new("Walker")
            .with_type(CardType::Planeswalker)
            .with_loyalty(3)
            .with_rules(
                serde_json::from_str::<Ability>(
                    r#"{"Loyalty":{"cost":{"Plus":1},"effects":[{"GainLife":{"amount":1}}]}}"#,
                )
                .unwrap(),
            ),
    )
}

/// "Planeswalkers you control have '[cost]: You gain 10 life.'"
fn giver(cost: &str) -> Arc<CardDefinition> {
    let rule = format!(
        r#"{{"Static":{{"affected":{{"Each":[{{"Type":"Planeswalker"}},{{"ControlledBy":"You"}}]}},"mods":[{{"GrantAbility":{{"Loyalty":{{"cost":{cost},"effects":[{{"GainLife":{{"amount":10}}}}]}}}}}}]}}}}"#
    );
    Arc::new(
        CardDefinition::new("Giver")
            .with_type(CardType::Artifact)
            .with_rules(serde_json::from_str::<Ability>(&rule).unwrap()),
    )
}

/// The granted abilities of the Walker offered now.
fn granted(driver: &Driver) -> Vec<mtg_engine::abilities::AbilityRef> {
    let Ok(decision) = driver.decision() else {
        return Vec::new();
    };
    let DecisionKind::Priority(options) = &decision.kind else {
        return Vec::new();
    };
    options
        .abilities
        .iter()
        .filter(|a| !matches!(a.source, AbilitySource::Printed))
        .copied()
        .collect()
}

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

fn loyalty(driver: &Driver) -> u32 {
    let id = driver.id_of("Walker").unwrap();
    driver
        .state()
        .object(id)
        .unwrap()
        .permanent()
        .unwrap()
        .counters_of(&CounterKind::Loyalty)
}

#[rstest]
#[case::zero(r#""Zero""#, 3)]
#[case::plus_two(r#"{"Plus":2}"#, 5)]
#[case::minus_one(r#"{"Minus":1}"#, 2)]
fn a_granted_loyalty_ability_is_activated(
    #[case] cost: &str,
    #[case] after: u32,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(seats).battlefield(0, [ready(walker()), ready(giver(cost))]),
    )?;
    let before = driver.state().player(pid(0)).unwrap().life();
    let offered = granted(&driver);
    assert_eq!(offered.len(), 1);
    driver.answer(Answer::Priority(PriorityAction::Activate(offered[0])))?;
    settle(&mut driver)?;
    assert_eq!(driver.state().player(pid(0)).unwrap().life() - before, 10);
    assert_eq!(loyalty(&driver), after);
    // One loyalty ability a turn: neither the printed nor the granted one is offered again.
    assert!(granted(&driver).is_empty());
    assert!(driver.offered_abilities("Walker")?.is_empty());
    Ok(())
}

#[rstest]
fn without_the_giver_nothing_is_granted(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let driver = Driver::from_scenario(Scenario::new(seats).battlefield(0, [ready(walker())]))?;
    assert!(granted(&driver).is_empty());
    assert_eq!(driver.offered_abilities("Walker")?, vec![0]);
    Ok(())
}
