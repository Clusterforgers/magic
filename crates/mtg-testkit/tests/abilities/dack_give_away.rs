//! Goal: the permanents that just arrived are given one to each opponent, a different opponent for each (Dack Fayden,
//! Helping Hand, `Effect::R1084ToDifferentOpponents`), at 2 to 4 seats; none stays with the controller, and the goad on
//! them holds under the new controller.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeSet;
use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::card::Card;
use mtg_engine::characteristics::controller_of;
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::turn::Step;
use mtg_testkit::cards::{free_creature, plains};
use mtg_testkit::{Driver, Scenario, ScriptError};
use rstest::rstest;

use crate::common::{on_battlefield, pid};

const RULES: &str = include_str!("../../../mtg-cards/cards/dack_fayden_helping_hand.json");

/// A free creature with the enters trigger of the rules file.
fn dack() -> Arc<CardDefinition> {
    let card: Card = serde_json::from_str(RULES).unwrap();
    let rule = serde_json::to_value(&card.faces[0].abilities[0]).unwrap();
    let ability: Ability = serde_json::from_value(rule).unwrap();
    Arc::new((*free_creature("Dack", 3, 2)).clone().with_rules(ability))
}

/// Seat 0 holds Dack in its first main phase; its library has a Plains above each of four creatures `Bear 0` to `Bear 3`.
fn table(seats: usize) -> Scenario {
    let mut library = Vec::new();
    for n in 0..4 {
        library.push(plains());
        library.push(free_creature(&format!("Bear {n}"), 2, 2));
    }
    let mut scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .hand(0, [dack()])
        .library(0, library);
    for seat in 1..seats {
        scenario = scenario.library(seat, (0..8).map(|_| plains()));
    }
    scenario
}

/// Plays on until the stack is empty, answering every question the default way.
fn settle(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..200 {
        if driver.state().stack().is_empty() && driver.state().resolving().is_none() {
            return Ok(());
        }
        if matches!(driver.decision()?.kind, DecisionKind::PayMana(_)) {
            driver.answer(Answer::PayMana(PayChoice::Auto))?;
        } else {
            driver.answer_passively()?;
        }
    }
    Err(ScriptError::Stuck("the stack".into()))
}

/// Answers passively until `seat` declares attackers, and returns the names of the creatures it must attack with.
fn required(driver: &mut Driver, seat: usize) -> Result<Vec<String>, ScriptError> {
    for _ in 0..400 {
        let decision = driver.decision()?.clone();
        if decision.player == pid(seat)
            && let DecisionKind::DeclareAttackers(options) = &decision.kind
        {
            return Ok(options
                .required
                .iter()
                .map(|id| driver.state().object(*id).unwrap().card().name.clone())
                .collect());
        }
        driver.answer(Driver::passive_answer(&decision))?;
    }
    Err(ScriptError::Stuck("declare-attackers".into()))
}

#[rstest]
fn each_opponent_gains_control_of_a_different_one(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table(seats))?;
    driver.cast("Dack")?;
    settle(&mut driver)?;
    let opponents = seats - 1;
    let mut owners = BTreeSet::new();
    for n in 0..opponents {
        let name = format!("Bear {n}");
        let id = on_battlefield(driver.state(), &name).expect(&name);
        let now = controller_of(driver.state(), id).unwrap();
        assert_ne!(now, pid(0), "{name} stayed with Dack's controller");
        owners.insert(now);
    }
    assert_eq!(
        owners.len(),
        opponents,
        "each opponent gains a different one"
    );
    assert!(on_battlefield(driver.state(), &format!("Bear {opponents}")).is_none());
    // The goad holds under the new controller: seat 1 must attack with the one it was given.
    let given: Vec<String> = (0..opponents)
        .map(|n| format!("Bear {n}"))
        .filter(|name| {
            let id = on_battlefield(driver.state(), name).unwrap();
            controller_of(driver.state(), id) == Some(pid(1))
        })
        .collect();
    assert_eq!(given.len(), 1);
    assert_eq!(required(&mut driver, 1)?, given);
    Ok(())
}
