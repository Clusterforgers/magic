//! Goal: "When the Nth <kind> counter is put on ~" triggers once, when counters of its kind take the permanent from below the number to it or past it (one by one or several at once), never for another kind and never again as more come, at tables of two to four seats; Robot Domination sacrifices itself and makes its Robots on its third plan counter; and the log replays to the same state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_core::ids::PlayerId;
use mtg_engine::decision::{Answer, PayChoice};
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::copying::replays;
use crate::common::names;

/// Answers passively until the stack and the waiting triggers are empty.
fn l291_settle(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..200 {
        let state = driver.state();
        if state.stack().is_empty()
            && state.resolving().is_none()
            && state.pending_triggers().is_empty()
        {
            return Ok(());
        }
        driver.answer_passively()?;
    }
    Err(ScriptError::Stuck("the stack".into()))
}

/// An artifact with "{0}: Put `step` `kind` counters on this artifact." and "When the third plan counter is put on this
/// artifact, draw a card."
fn l291_board(step: u32, kind: &str) -> Arc<CardDefinition> {
    let put: Ability = serde_json::from_str(&format!(
        r#"{{"Activated":{{"cost":"{{0}}","effects":[{{"PutCounters":{{"on":"This","counter":{{"Other":"{kind}"}},"count":{step}}}}}]}}}}"#
    ))
    .unwrap();
    let third: Ability = serde_json::from_str(
        r#"{"Triggered":{"when":{"NthCounterPut":{"counter":{"Other":"plan"},"number":3}},"effects":[{"Draw":{}}]}}"#,
    )
    .unwrap();
    Arc::new(
        CardDefinition::new("Plan Board")
            .with_type(CardType::Artifact)
            .with_rules(put)
            .with_rules(third),
    )
}

/// Seat 0 controls `permanents` at its first main phase with a library of ten Plan Boards; every seat has a library.
fn l291_table(seats: usize, permanents: Vec<Arc<CardDefinition>>) -> Scenario {
    let mut scenario = Scenario::new(seats)
        .at(2, 0, Step::Main1)
        .battlefield(0, permanents.into_iter().map(ready));
    for seat in 0..seats {
        scenario = scenario.library(seat, (0..10).map(|_| l291_board(1, "plan")));
    }
    scenario
}

#[rstest]
#[case::one_at_a_time(1, "plan", &[0, 0, 1, 1, 1])]
#[case::two_at_a_time(2, "plan", &[0, 1, 1])]
#[case::three_at_once(3, "plan", &[1, 1])]
#[case::past_it_at_once(5, "plan", &[1, 1])]
#[case::another_kind(3, "charge", &[0, 0])]
fn l291_the_nth_counter_triggers_once(
    #[values(2, 3, 4)] seats: usize,
    #[case] step: u32,
    #[case] kind: &str,
    #[case] draws: &[usize],
) -> Result<(), ScriptError> {
    let scenario = l291_table(seats, vec![l291_board(step, kind)]);
    let setup = scenario.clone().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario)?;
    let hand = Zone::Hand(PlayerId(0));
    let before = driver.state().zone(hand).len();
    for (activation, drawn) in draws.iter().enumerate() {
        driver.begin_activation("Plan Board")?;
        driver.answer(Answer::PayMana(PayChoice::Auto))?;
        l291_settle(&mut driver)?;
        assert_eq!(
            driver.state().zone(hand).len() - before,
            *drawn,
            "after activation {activation}"
        );
    }
    replays(&driver, &setup);
    Ok(())
}

/// An artifact with "{0}: Put a plan counter on target permanent."
fn l291_pusher() -> Arc<CardDefinition> {
    let put: Ability = serde_json::from_str(
        r#"{"Activated":{"cost":"{0}","targets":[{"what":{"Object":[]}}],"effects":[{"PutCounters":{"on":{"Target":0},"counter":{"Other":"plan"}}}]}}"#,
    )
    .unwrap();
    Arc::new(
        CardDefinition::new("Plan Pusher")
            .with_type(CardType::Artifact)
            .with_rules(put),
    )
}

#[rstest]
fn l291_robot_domination_completes_its_plan(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = l291_table(
        seats,
        vec![card("Robot Domination").unwrap(), l291_pusher()],
    );
    let setup = scenario.clone().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario)?;
    let robots = |driver: &Driver| {
        names(driver.state(), Zone::Battlefield)
            .iter()
            .filter(|name| name.as_str() == "Robot Villain")
            .count()
    };
    for counter in 1..=3 {
        driver.begin_activation("Plan Pusher")?;
        let target = driver.target_named("Robot Domination")?;
        driver.choose_targets(&[target])?;
        driver.answer(Answer::PayMana(PayChoice::Auto))?;
        l291_settle(&mut driver)?;
        let done = counter == 3;
        assert_eq!(
            robots(&driver),
            if done { 3 } else { 0 },
            "counter {counter}"
        );
        let graveyard = names(driver.state(), Zone::Graveyard(PlayerId(0)));
        assert_eq!(graveyard.contains(&"Robot Domination".to_owned()), done);
    }
    replays(&driver, &setup);
    Ok(())
}
