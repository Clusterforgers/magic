//! Goal: the shields of the next damage that change it in part ("prevent half that damage, rounded down", "prevent all
//! but 1 of that damage") play through the whole stack at 2 to 4 seats: the chosen source's next damage is lessened as
//! they say, even when that leaves it whole, the shield then ends, and the game replays to the same state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use crate::common;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_core::ids::ObjectId;
use mtg_engine::choose::{CardsChoice, ReplacementChoice};
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::replay;
use mtg_engine::rules::Rule;
use mtg_testkit::cards::{burn, cost, mountain};
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// An instant for nothing that chooses a source and changes the next damage it deals to you as `by` says.
fn ward(by: &str) -> Arc<CardDefinition> {
    let rules = format!(
        r#"{{"Spell":{{"effects":[
            {{"Choose":{{"from":{{"Each":[{{"AnyOf":[{{"In":"Battlefield"}},{{"In":"Stack"}},{{"In":"Command"}}]}}]}},"as":"source"}}}},
            {{"Shield":{{"when":{{"Damage":{{"to":"You","source":[{{"Is":{{"Bound":"source"}}}}]}}}},"by":{by},"duration":"UntilEndOfTurn"}}}}
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

fn table(by: &str, seats: usize) -> Scenario {
    let library = || {
        (0..10)
            .map(|_| mtg_testkit::cards::forest())
            .collect::<Vec<_>>()
    };
    (0..seats)
        .fold(Scenario::new(seats), |scenario, seat| {
            scenario.library(seat, library())
        })
        .hand(0, vec![ward(by)])
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

const HALF: &str = r#""PreventHalfNext""#;
const ALL_BUT_ONE: &str = r#"{"PreventAllButNext":1}"#;

#[rstest]
#[case::half_of_three_rounded_down(HALF, "Spark0", 18)]
#[case::half_of_two(HALF, "Spark1", 19)]
#[case::all_but_one_of_three(ALL_BUT_ONE, "Spark0", 19)]
#[case::all_but_one_of_two(ALL_BUT_ONE, "Spark1", 19)]
fn the_next_damage_of_the_chosen_source_is_lessened_and_the_shield_ends(
    #[values(2, 3, 4)] seats: usize,
    #[case] by: &str,
    #[case] spark: &str,
    #[case] life_left: i32,
) -> Result<(), ScriptError> {
    let setup = table(by, seats).setup().unwrap();
    let mut driver = Driver::from_scenario(table(by, seats))?;
    let me = driver.target_player(0);
    driver.pass()?;
    driver.cast_at(spark, &[me])?;
    back_to_me(&mut driver)?;
    driver.cast("Ward")?;
    settle(&mut driver, Some(spark))?;
    assert_eq!((life(&driver), shields(&driver)), (life_left, 0));
    let replayed = replay(&setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
    Ok(())
}

#[rstest]
#[case::half(HALF, 1)]
#[case::all_but_one(ALL_BUT_ONE, 1)]
fn a_damage_too_small_to_lessen_still_uses_the_shield(
    #[values(2, 4)] seats: usize,
    #[case] by: &str,
    #[case] dealt: i32,
) -> Result<(), ScriptError> {
    // "Ember" deals 1: half of 1 rounded down is nothing, and all but 1 of 1 is all of it; the shield is still spent.
    let table = table(by, seats).hand(1, vec![burn("Ember", 1), burn("Spark1", 2)]);
    let mut driver = Driver::from_scenario(table)?;
    let me = driver.target_player(0);
    driver.pass()?;
    driver.cast_at("Ember", &[me])?;
    back_to_me(&mut driver)?;
    driver.cast("Ward")?;
    settle(&mut driver, Some("Ember"))?;
    assert_eq!((life(&driver), shields(&driver)), (20 - dealt, 0));
    Ok(())
}
