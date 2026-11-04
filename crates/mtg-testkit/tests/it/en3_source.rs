//! Goal: a choice of a source and the shield that holds it ("the next time a source of your choice would deal damage to you
//! this turn, prevent that damage") play through the whole stack: the shield prevents the damage of the chosen source once
//! and ends, damage of another source passes, a choice with no legal source leaves no shield and no open question, and the
//! game replays to the same state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use crate::common;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_core::ids::ObjectId;
use mtg_engine::choose::{CardsChoice, ReplacementChoice};
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::rules::Rule;
use mtg_engine::{Event, replay};
use mtg_testkit::cards::{burn, cost, mountain};
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// An instant for nothing that chooses a source (restricted to `restrict`) and prevents the next damage it deals to you.
fn ward(restrict: &str) -> Arc<CardDefinition> {
    let rules = format!(
        r#"{{"Spell":{{"effects":[
            {{"Choose":{{"from":{{"Each":[{{"AnyOf":[{{"In":"Battlefield"}},{{"In":"Stack"}},{{"In":"Command"}}]}}{restrict}]}},"as":"source"}}}},
            {{"Shield":{{"when":{{"Damage":{{"to":"You","source":[{{"Is":{{"Bound":"source"}}}}]}}}},"by":"PreventNext","duration":"UntilEndOfTurn"}}}}
        ]}}}}"#
    );
    let rules: Ability = serde_json::from_str(&rules).unwrap();
    Arc::new(
        CardDefinition::new("Ward")
            .with_type(CardType::Instant)
            .with_mana_cost(cost(0, &[]))
            .with_rules(rules),
    )
}

fn table(restrict: &str, seats: usize) -> Scenario {
    let library = || {
        (0..10)
            .map(|_| mtg_testkit::cards::forest())
            .collect::<Vec<_>>()
    };
    (0..seats)
        .fold(Scenario::new(seats), |scenario, seat| {
            scenario.library(seat, library())
        })
        .hand(0, vec![ward(restrict)])
        .hand(1, vec![burn("Spark0", 3), burn("Spark1", 2)])
        .battlefield(1, (0..2).map(|_| ready(mountain())))
}

/// Passes priority until the stack is empty, answering each question: the chosen source is the one named.
fn settle(driver: &mut Driver, chosen: Option<&str>) -> Result<(), ScriptError> {
    for _ in 0..200 {
        let Some(decision) = driver.engine().decision().cloned() else {
            return Ok(());
        };
        match decision.kind {
            DecisionKind::ChooseReplacement(_) => {
                driver.answer(Answer::ChooseReplacement(ReplacementChoice(0)))?;
            }
            DecisionKind::ChooseCards(options) => {
                let named = |id: &ObjectId| {
                    chosen.is_some_and(|name| {
                        driver
                            .state()
                            .object(*id)
                            .is_some_and(|object| object.card().name == name)
                    })
                };
                let pick: Vec<ObjectId> =
                    options.from.iter().copied().filter(named).take(1).collect();
                driver.answer(Answer::ChooseCards(CardsChoice(pick)))?;
            }
            DecisionKind::Priority(_)
                if driver.state().stack().is_empty() && driver.state().resolving().is_none() =>
            {
                return Ok(());
            }
            DecisionKind::Priority(_) => {
                driver.pass()?;
            }
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    Err(ScriptError::Stuck("the stack".into()))
}

/// Passes priority for the other seats until seat 0 holds it again, with the spell still on the stack.
fn back_to_me(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..8 {
        if driver.decision()?.player == common::pid(0) {
            return Ok(());
        }
        driver.pass()?;
    }
    Err(ScriptError::Stuck("priority".into()))
}

fn life(driver: &Driver) -> i32 {
    driver.state().player(common::pid(0)).unwrap().life()
}

fn shields(driver: &Driver) -> usize {
    driver
        .state()
        .rule_effects()
        .iter()
        .filter(|effect| matches!(effect.rule, Rule::PreventDamage(_)))
        .count()
}

#[rstest]
#[case::the_chosen_source_is_prevented_once("Spark0", 20, 0)]
#[case::another_source_is_not_prevented("Mountain", 17, 1)]
fn the_shield_prevents_the_damage_of_the_chosen_source_only(
    #[values(2, 3, 4)] seats: usize,
    #[case] chosen: &str,
    #[case] life_left: i32,
    #[case] shields_left: usize,
) -> Result<(), ScriptError> {
    let setup = table("", seats).setup().unwrap();
    let mut driver = Driver::from_scenario(table("", seats))?;
    let me = driver.target_player(0);
    driver.pass()?;
    driver.cast_at("Spark0", &[me])?;
    back_to_me(&mut driver)?;
    driver.cast("Ward")?;
    settle(&mut driver, Some(chosen))?;
    assert_eq!(life(&driver), life_left);
    assert_eq!(shields(&driver), shields_left);
    let replayed = replay(&setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
    Ok(())
}

#[test]
fn the_shield_ends_with_the_first_damage_it_prevents() -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table("", 2))?;
    let me = driver.target_player(0);
    driver.pass()?;
    driver.cast_at("Spark0", &[me])?;
    back_to_me(&mut driver)?;
    driver.cast("Ward")?;
    settle(&mut driver, Some("Spark0"))?;
    assert_eq!((life(&driver), shields(&driver)), (20, 0));
    // Damage after the shield served its one event is dealt.
    driver.pass()?;
    driver.cast_at("Spark1", &[me])?;
    settle(&mut driver, None)?;
    assert_eq!(life(&driver), 18);
    Ok(())
}

#[test]
fn a_choice_with_no_legal_source_leaves_no_shield_and_no_open_question() -> Result<(), ScriptError>
{
    // Nothing is green: the choice has no option, the shield names nothing, and the game goes on.
    let restrict = r#",{"Color":"Green"}"#;
    let mut driver = Driver::from_scenario(table(restrict, 2))?;
    let me = driver.target_player(0);
    driver.pass()?;
    driver.cast_at("Spark0", &[me])?;
    back_to_me(&mut driver)?;
    driver.cast("Ward")?;
    settle(&mut driver, None)?;
    assert_eq!(shields(&driver), 0);
    assert_eq!(life(&driver), 17);
    assert!(
        driver
            .engine()
            .events()
            .iter()
            .all(|event| !matches!(event, Event::ShieldUsed { .. }))
    );
    Ok(())
}
