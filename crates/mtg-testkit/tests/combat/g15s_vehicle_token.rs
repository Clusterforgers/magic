//! Goal: a Vehicle token made by a spell keeps its printed power and toughness without being a creature, is crewed like a Vehicle card and is an artifact creature until end of turn, at tables of two, three and four seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_engine::characteristics;
use mtg_engine::choose::CardsChoice;
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_testkit::cards::{cost, free_creature};
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// A free creature that makes a 3/2 Vehicle token with Crew 1 when it enters.
fn maker() -> Arc<CardDefinition> {
    let rules: Ability = serde_json::from_str(
        r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"effects":[{"CreateToken":{"token":{"Custom":{"name":"Vehicle","types":["Artifact"],"subtypes":["Vehicle"],"power":3,"toughness":2,"abilities":[{"Keyword":{"Crew":1}}]}}}}]}}"#,
    )
    .unwrap();
    Arc::new(
        CardDefinition::new("Maker")
            .with_mana_cost(cost(0, &[]))
            .with_type(CardType::Creature)
            .with_power_toughness(1, 1)
            .with_rules(rules),
    )
}

/// Answers passively until the stack is empty and nothing resolves.
fn settle(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..200 {
        if driver.state().stack().is_empty() && driver.state().resolving().is_none() {
            return Ok(());
        }
        driver.answer_passively()?;
    }
    Err(ScriptError::Stuck("the stack".into()))
}

fn table(seats: usize) -> Scenario {
    Scenario::new(seats)
        .hand(0, [maker()])
        .battlefield(0, [ready(free_creature("Crew", 1, 1))])
}

#[rstest]
fn the_token_is_a_vehicle_with_printed_stats_until_it_is_crewed(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table(seats))?;
    driver.cast("Maker")?;
    settle(&mut driver)?;
    let token = characteristics(driver.state(), driver.id_of("Vehicle")?).unwrap();
    assert!(!token.is_creature());
    assert_eq!((token.power, token.toughness), (Some(3), Some(2)));

    driver.begin_activation("Vehicle")?;
    if matches!(driver.decision()?.kind, DecisionKind::ChooseCards(_)) {
        let crew = driver.id_of("Crew")?;
        driver.answer(Answer::ChooseCards(CardsChoice(vec![crew])))?;
    }
    settle(&mut driver)?;
    let token = characteristics(driver.state(), driver.id_of("Vehicle")?).unwrap();
    assert!(token.is_creature());
    assert_eq!((token.power, token.toughness), (Some(3), Some(2)));
    Ok(())
}
