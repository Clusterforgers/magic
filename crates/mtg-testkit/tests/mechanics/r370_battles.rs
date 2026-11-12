//! Goal: "if an opponent protects it" (`Pred::ProtectedBy`, Portent Tracker) removes a defense counter from a battle an
//! opponent protects and puts one on a battle its controller protects, at any table size.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::value::CounterKind;
use mtg_core::card::CardType;
use mtg_core::ids::ObjectId;
use mtg_engine::decision::{Answer, DecisionKind, PayChoice, PriorityAction};
use mtg_testkit::corpus::card;
use mtg_testkit::scenario::{protected_by, with_counters};
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// A battle with no rules of its own.
fn battle() -> Arc<CardDefinition> {
    Arc::new(CardDefinition::new("Front").with_type(CardType::Battle))
}

fn defense(driver: &Driver, id: ObjectId) -> u32 {
    driver
        .state()
        .object(id)
        .and_then(|object| object.permanent())
        .map_or(0, |permanent| {
            permanent.counters_of(&CounterKind::defense())
        })
}

/// Passes priority until the stack is empty, answering anything else passively.
fn settle(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..100 {
        let decision = driver.decision()?.clone();
        match decision.kind {
            DecisionKind::Priority(_)
                if driver.state().stack().is_empty() && driver.state().resolving().is_none() =>
            {
                return Ok(());
            }
            DecisionKind::Priority(_) => {
                driver.pass()?;
            }
            DecisionKind::PayMana(_) => {
                driver.answer(Answer::PayMana(PayChoice::Auto))?;
            }
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    Err(ScriptError::Stuck("the stack".into()))
}

#[rstest]
#[case::an_opponent_protects_it(true, 3)]
#[case::you_protect_it(false, 5)]
fn r370_a_defense_counter_by_protector(
    #[values(2, 3, 4)] seats: usize,
    #[case] opponent: bool,
    #[case] after: u32,
) -> Result<(), ScriptError> {
    let protector = if opponent { seats - 1 } else { 0 };
    let front = protected_by(
        with_counters(ready(battle()), CounterKind::defense(), 4),
        protector,
    );
    let mut driver = Driver::from_scenario(
        Scenario::new(seats).battlefield(0, [ready(card("Portent Tracker").unwrap()), front]),
    )?;
    let DecisionKind::Priority(options) = &driver.decision()?.kind else {
        panic!("priority was expected");
    };
    let tracker = driver.id_of("Portent Tracker")?;
    let ability = options
        .abilities
        .iter()
        .find(|ability| ability.object == tracker && ability.ability == 1)
        .copied()
        .unwrap();
    driver.answer(Answer::Priority(PriorityAction::Activate(ability)))?;
    let front = driver.target_named("Front")?;
    driver.choose_targets(&[front])?;
    settle(&mut driver)?;
    let id = driver.id_of("Front")?;
    assert_eq!(defense(&driver, id), after);
    Ok(())
}
