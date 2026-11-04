//! Goal: an ability limited to once each turn is offered again on the next turn and one limited to once each game never is, at tables of two and three seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_engine::decision::DecisionKind;
use mtg_engine::turn::Step;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::name_of;
use crate::common::removal::resolve_the_stack;

/// An artifact whose only ability gains 1 life for free, limited as `limit` says.
fn source(limit: &str) -> Arc<CardDefinition> {
    let json = format!(
        r#"{{"Activated":{{"cost":[],"limit":"{limit}","effects":[{{"GainLife":{{"amount":1}}}}]}}}}"#
    );
    let rules: Ability = serde_json::from_str(&json).unwrap();
    Arc::new(
        CardDefinition::new("Spring")
            .with_type(CardType::Artifact)
            .with_rules(rules),
    )
}

fn offered(driver: &Driver) -> bool {
    let DecisionKind::Priority(options) = &driver.decision().unwrap().kind else {
        return false;
    };
    options
        .abilities
        .iter()
        .any(|ability| name_of(driver.state(), ability.object) == "Spring")
}

/// Passes priority until a later turn of seat 0 holds priority in its first main phase.
fn to_my_next_turn(driver: &mut Driver) -> Result<(), ScriptError> {
    let turn = driver.state().turn().number();
    for _ in 0..200 {
        let state = driver.state();
        if state.turn().number() > turn
            && state.turn().active().0 == 0
            && state.turn().step() == Step::Main1
            && matches!(driver.decision()?.kind, DecisionKind::Priority(_))
        {
            return Ok(());
        }
        driver.pass()?;
    }
    panic!("the turn never came");
}

#[rstest]
#[case::each_turn("OncePerTurn", true)]
#[case::each_game("OncePerGame", false)]
fn a_limit_ends_with_the_turn_or_the_game(
    #[case] limit: &str,
    #[case] again_next_turn: bool,
    #[values(2, 3)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(seats)
            .at(1, 0, Step::Main1)
            .battlefield(0, [ready(source(limit))]),
    )?;
    assert!(offered(&driver));
    driver.begin_activation("Spring")?;
    resolve_the_stack(&mut driver)?;
    driver.expect_life(0, 21)?;
    assert!(!offered(&driver), "used up this turn");
    to_my_next_turn(&mut driver)?;
    assert_eq!(offered(&driver), again_next_turn);
    Ok(())
}
