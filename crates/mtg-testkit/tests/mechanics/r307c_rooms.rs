//! Goal: Polluted Cistern, cast as its door (the only one its two Swamps pay for), makes each opponent lose 1 life for each
//! card type among the cards milled from its controller's library into their graveyard, once for the batch, at tables of
//! two, three and four seats; a mill of one type costs 1 life, of two types 2, and an opponent's mill costs nothing; and
//! the log replays to the state it was played to.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_engine::decision::DecisionKind;
use mtg_engine::replay;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{bear, cost, swamp};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::casting::pay;
use crate::common::pid;

const R307C_ROOM: &str = "Polluted Cistern // Dim Oubliette";

/// A free sorcery whose one effect is given as JSON.
fn r307c_sorcery(effect: &str) -> Arc<CardDefinition> {
    let ability: Ability =
        serde_json::from_str(&format!(r#"{{"Spell":{{"effects":[{effect}]}}}}"#)).unwrap();
    Arc::new(
        CardDefinition::new("Free Mill")
            .with_type(CardType::Sorcery)
            .with_mana_cost(cost(0, &[]))
            .with_rules(ability),
    )
}

/// Passes or answers passively until seat 0 holds priority on an empty stack.
fn r307c_settle(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..40 {
        let decision = driver.decision()?;
        if decision.player == pid(0)
            && matches!(decision.kind, DecisionKind::Priority(_))
            && driver.state().zone(Zone::Stack).is_empty()
        {
            return Ok(());
        }
        if matches!(decision.kind, DecisionKind::Priority(_)) {
            driver.pass()?;
        } else {
            driver.answer_passively()?;
        }
    }
    Err(ScriptError::Stuck("the stack does not settle".to_owned()))
}

fn r307c_life(driver: &Driver, seat: usize) -> i32 {
    driver.state().player(pid(seat)).unwrap().life()
}

/// `mixed`: every library holds Bears and Swamps by turns (two types among any three), else only Bears.
#[rstest]
#[case::one_type(false, r#"{"Mill":{"count":3}}"#, 1)]
#[case::two_types(true, r#"{"Mill":{"count":3}}"#, 2)]
#[case::an_opponent_mills(true, r#"{"Mill":{"who":"Opponents","count":3}}"#, 0)]
fn r307c_cistern_counts_the_card_types_milled(
    #[values(2, 3, 4)] seats: usize,
    #[case] mixed: bool,
    #[case] mill: &str,
    #[case] lost: i32,
) -> Result<(), ScriptError> {
    let library = |_| -> Vec<Arc<CardDefinition>> {
        (0..12)
            .map(|at| {
                if mixed && at % 2 == 1 {
                    swamp()
                } else {
                    bear()
                }
            })
            .collect()
    };
    let mut scenario = Scenario::new(seats)
        .battlefield(0, (0..2).map(|_| ready(swamp())))
        .hand(0, [card(R307C_ROOM).unwrap(), r307c_sorcery(mill)]);
    for seat in 0..seats {
        scenario = scenario.library(seat, library(seat));
    }
    let mut driver = Driver::from_scenario(scenario)?;
    driver.begin_cast(R307C_ROOM)?;
    pay(&mut driver)?;
    r307c_settle(&mut driver)?;
    let before: Vec<i32> = (0..seats).map(|seat| r307c_life(&driver, seat)).collect();
    driver.cast("Free Mill")?;
    r307c_settle(&mut driver)?;
    assert_eq!(r307c_life(&driver, 0), before[0]);
    for (seat, was) in before.iter().enumerate().skip(1) {
        assert_eq!(r307c_life(&driver, seat), was - lost, "seat {seat}");
    }
    let state = replay(driver.engine().setup(), driver.engine().events()).unwrap();
    assert_eq!(state.state_hash(), driver.state().state_hash());
    Ok(())
}
