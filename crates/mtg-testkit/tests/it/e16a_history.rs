//! Goal: pack e16a's facts of the turn's history play whole at two, three and four seats: an ability that asks for "an
//! artifact you've sacrificed this turn" is offered after an artifact is sacrificed and not after a creature is, judged
//! as the permanent was on the battlefield.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::turn::Step;
use mtg_testkit::cards::forest;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// A permanent of the type with the ability `json`.
fn permanent(name: &str, kind: CardType, json: &str) -> Arc<CardDefinition> {
    let rule: Ability = serde_json::from_str(json).unwrap();
    let mut card = CardDefinition::new(name).with_type(kind).with_rules(rule);
    if kind == CardType::Creature {
        card = card.with_power_toughness(1, 1);
    }
    Arc::new(card)
}

const SAC_SELF: &str =
    r#"{"Activated":{"cost":["SacrificeSelf"],"effects":[{"GainLife":{"amount":1}}]}}"#;
const ASKS: &str = r#"{"Activated":{"condition":{"Compare":{"left":{"Tally":{"event":"Sacrificed","filter":[{"Type":"Artifact"}],"who":"You"}},"op":"Ge","right":1}},"cost":["Tap"],"effects":[{"GainLife":{"amount":3}}]}}"#;

/// Seat 0 with the asking artifact, an artifact and a creature that sacrifice themselves, at `seats` seats.
fn driver(seats: usize) -> Result<Driver, ScriptError> {
    let scenario = (0..seats).fold(
        Scenario::new(seats)
            .battlefield(
                0,
                vec![
                    ready(permanent("Satchel", CardType::Artifact, ASKS)),
                    ready(permanent("Bauble", CardType::Artifact, SAC_SELF)),
                    ready(permanent("Bear", CardType::Creature, SAC_SELF)),
                ],
            )
            .at(1, 0, Step::Main1),
        |scenario, seat| scenario.library(seat, vec![forest(); 5]),
    );
    Driver::from_scenario(scenario)
}

/// Activates the named permanent's ability and lets it resolve.
fn sacrifice(driver: &mut Driver, name: &str) -> Result<(), ScriptError> {
    driver.begin_activation(name)?;
    if matches!(driver.decision()?.kind, DecisionKind::PayMana(_)) {
        driver.answer(Answer::PayMana(PayChoice::Auto))?;
    }
    while !driver.state().stack().is_empty() {
        driver.pass()?;
    }
    Ok(())
}

#[rstest]
#[case::nothing(None, false)]
#[case::an_artifact(Some("Bauble"), true)]
#[case::a_creature(Some("Bear"), false)]
fn an_artifact_sacrificed_this_turn_opens_the_ability(
    #[values(2, 3, 4)] seats: usize,
    #[case] sacrificed: Option<&str>,
    #[case] offered: bool,
) -> Result<(), ScriptError> {
    let mut driver = driver(seats)?;
    if let Some(name) = sacrificed {
        sacrifice(&mut driver, name)?;
    }
    assert_eq!(!driver.offered_abilities("Satchel")?.is_empty(), offered);
    Ok(())
}
