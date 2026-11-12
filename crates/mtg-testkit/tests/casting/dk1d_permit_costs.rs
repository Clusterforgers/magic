//! Goal: the costs a cast permission adds join the way the card is cast through it: added mana joins the mana of the way (a
//! free cast pays it too), added life is taken with the cast, and a permission that adds nothing is used before one that
//! adds a cost.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_core::card::CardType;
use mtg_engine::decision::{Answer, PayChoice};
use mtg_engine::turn::Step;
use mtg_testkit::cards::{creature, mountain};
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::{on_battlefield, pid};

/// A permanent whose permission to cast creature cards from the graveyard has the fields `body`.
fn dk1d_key(body: &str) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new("Key")
            .with_type(CardType::Artifact)
            .with_rules(
                serde_json::from_str(&format!(
                    r#"{{"Rule":{{"rule":{{"MayCast":{{"from":"Graveyard","what":[{{"Type":"Creature"}}],{body}}}}}}}}}"#
                ))
                .unwrap(),
            ),
    )
}

/// Seat 0 holds the keys and `mountains` Mountains, with a Bear (cost 2) in its graveyard.
fn dk1d_keyed(keys: &[&str], mountains: usize) -> Scenario {
    let lands = (0..mountains).map(|_| ready(mountain()));
    let keys = keys.iter().map(|body| ready(dk1d_key(body)));
    Scenario::new(2)
        .at(1, 0, Step::Main1)
        .graveyard(0, [creature("Bear", 2, &[], 2, 2)])
        .battlefield(0, lands.chain(keys))
}

/// The life of seat 0.
fn dk1d_life(driver: &Driver) -> i32 {
    driver.state().player(pid(0)).unwrap().life()
}

/// Passes until the stack is empty.
fn dk1d_settle(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..40 {
        if driver.state().stack().is_empty() && driver.state().resolving().is_none() {
            return Ok(());
        }
        driver.answer_passively()?;
    }
    Err(ScriptError::Stuck("the stack".into()))
}

#[rstest]
#[case::two_lands_are_short(r#""additional":[{"Mana":"{1}"}]"#, 2, None)]
#[case::mana_joins_the_cost(r#""additional":[{"Mana":"{1}"}]"#, 3, Some("{3}"))]
#[case::a_free_cast_pays_the_added_mana(
    r#""free":true,"additional":[{"Mana":"{1}"}]"#,
    1,
    Some("{1}")
)]
#[case::a_free_cast_with_no_land_for_added_mana(
    r#""free":true,"additional":[{"Mana":"{1}"}]"#,
    0,
    None
)]
fn dk1d_the_mana_of_an_added_cost_joins_the_mana_of_the_way(
    #[case] body: &str,
    #[case] mountains: usize,
    #[case] cost: Option<&str>,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(dk1d_keyed(&[body], mountains))?;
    match cost {
        None => assert!(driver.begin_cast("Bear").is_err()),
        Some(cost) => {
            driver.begin_cast("Bear")?;
            assert_eq!(driver.pay_options()?.cost.to_string(), cost);
        }
    }
    Ok(())
}

#[test]
fn dk1d_a_permission_that_adds_life_takes_it_with_the_cast() -> Result<(), ScriptError> {
    let body = r#""free":true,"additional":[{"PayLife":2}]"#;
    let mut driver = Driver::from_scenario(dk1d_keyed(&[body], 0))?;
    driver.begin_cast("Bear")?;
    driver.expect_decision(0, "pay-mana")?;
    assert_eq!(driver.pay_options()?.cost.to_string(), "{0}");
    driver.answer(Answer::PayMana(PayChoice::Auto))?;
    dk1d_settle(&mut driver)?;
    assert_eq!(dk1d_life(&driver), 18);
    assert!(on_battlefield(driver.state(), "Bear").is_some());
    Ok(())
}

#[test]
fn dk1d_a_permission_that_adds_nothing_is_used_before_one_that_adds_a_cost()
-> Result<(), ScriptError> {
    let costly = r#""additional":[{"PayLife":2}]"#;
    let mut driver = Driver::from_scenario(dk1d_keyed(&[costly, r#""flash":false"#], 2))?;
    driver.begin_cast("Bear")?;
    driver.expect_decision(0, "pay-mana")?;
    driver.answer(Answer::PayMana(PayChoice::Auto))?;
    assert_eq!(dk1d_life(&driver), 20, "the plain permission asks no life");
    Ok(())
}
