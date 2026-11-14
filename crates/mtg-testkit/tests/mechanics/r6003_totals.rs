//! Goal: a search or choice "with total mana value N or less" or "with total power X or less" offers the cards with a
//! budget at tables of two to four seats: an answer above the total is refused, one within it is kept, and X read from a
//! creature that died is its power as it last existed (Protean Hulk, Moorland Rescuer).
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_core::mana::ManaSymbol;
use mtg_engine::choose::CardsChoice;
use mtg_engine::decision::{Answer, Decision, DecisionKind, PayChoice};
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{burn, creature_costing, mountain};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::casting::lands;
use crate::common::{name_of, pid};

/// Answers passively until a choice of cards with a budget is asked, and returns it.
fn r6003_until_budget(driver: &mut Driver) -> Result<Decision, ScriptError> {
    for _ in 0..64 {
        let decision = driver.decision()?.clone();
        if let DecisionKind::ChooseCards(options) = &decision.kind
            && options.budget.is_some()
        {
            return Ok(decision);
        }
        let answer = match &decision.kind {
            DecisionKind::PayMana(_) => Answer::PayMana(PayChoice::Auto),
            _ => Driver::passive_answer(&decision),
        };
        driver.answer(answer)?;
    }
    Err(ScriptError::Stuck(
        "no choice with a total was asked".into(),
    ))
}

/// The ids of the cards of the choice by name, in the order named.
fn r6003_ids(driver: &Driver, decision: &Decision, names: &[&str]) -> Vec<mtg_core::ids::ObjectId> {
    let DecisionKind::ChooseCards(options) = &decision.kind else {
        panic!("a choice of cards");
    };
    names
        .iter()
        .map(|name| {
            *options
                .from
                .iter()
                .find(|id| name_of(driver.state(), **id) == *name)
                .unwrap_or_else(|| panic!("{name} is offered"))
        })
        .collect()
}

/// A vanilla 1/1 creature card for `generic` colorless mana and a given power.
fn r6003_creature(
    name: &str,
    generic: u32,
    power: i32,
) -> std::sync::Arc<mtg_cards::CardDefinition> {
    creature_costing(name, &[ManaSymbol::Generic(generic)], power, 1)
}

/// Seat 0's `dier` on the battlefield is dealt 7 by a bolt; the dies trigger asks its choice with a budget.
fn r6003_kill(seats: usize, dier: &str, library: bool) -> Result<(Driver, Decision), ScriptError> {
    let cards = vec![
        r6003_creature("Two", 2, 1),
        r6003_creature("Three", 3, 1),
        r6003_creature("Four", 4, 3),
        r6003_creature("Five", 5, 2),
    ];
    let mut permanents = lands(1, mountain);
    permanents.push(ready(card(dier).unwrap()));
    let mut scenario = Scenario::new(seats)
        .battlefield(0, permanents)
        .hand(0, vec![burn("Bolt", 7)]);
    scenario = if library {
        scenario.library(0, cards)
    } else {
        scenario.graveyard(0, cards)
    };
    let mut driver = Driver::from_scenario(scenario)?;
    let target = driver.target_named(dier)?;
    driver.cast_at("Bolt", &[target])?;
    let decision = r6003_until_budget(&mut driver)?;
    Ok((driver, decision))
}

#[rstest]
#[case::two_seats(2)]
#[case::three_seats(3)]
#[case::four_seats(4)]
fn r6003_hulk_finds_creatures_within_six(#[case] seats: usize) -> Result<(), ScriptError> {
    let (mut driver, decision) = r6003_kill(seats, "Protean Hulk", true)?;
    let DecisionKind::ChooseCards(options) = &decision.kind else {
        panic!("a choice of cards");
    };
    let budget = options.budget.clone().unwrap();
    assert_eq!(budget.most, 6);
    let over = r6003_ids(&driver, &decision, &["Four", "Three"]);
    assert_eq!(budget.total(&options.from, &over), 7);
    assert!(
        driver
            .answer(Answer::ChooseCards(CardsChoice(over)))
            .is_err(),
        "7 is above the total"
    );
    let within = r6003_ids(&driver, &decision, &["Four", "Two"]);
    driver.answer(Answer::ChooseCards(CardsChoice(within)))?;
    let state = driver.state();
    let mut entered: Vec<String> = state
        .zone(Zone::Battlefield)
        .iter()
        .map(|id| name_of(state, *id))
        .filter(|name| ["Two", "Three", "Four", "Five"].contains(&name.as_str()))
        .collect();
    entered.sort();
    assert_eq!(entered, ["Four", "Two"]);
    assert_eq!(state.zone(Zone::Library(pid(0))).len(), 2);
    Ok(())
}

#[rstest]
#[case::two_seats(2)]
#[case::four_seats(4)]
fn r6003_rescuer_returns_power_up_to_its_own(#[case] seats: usize) -> Result<(), ScriptError> {
    let (mut driver, decision) = r6003_kill(seats, "Moorland Rescuer", false)?;
    let DecisionKind::ChooseCards(options) = &decision.kind else {
        panic!("a choice of cards");
    };
    // X is the Rescuer's power as it last existed: 4.
    let budget = options.budget.clone().unwrap();
    assert_eq!(budget.most, 4);
    // The Rescuer itself is not offered ("other").
    assert!(
        options
            .from
            .iter()
            .all(|id| name_of(driver.state(), *id) != "Moorland Rescuer")
    );
    let over = r6003_ids(&driver, &decision, &["Four", "Five"]);
    assert!(
        driver
            .answer(Answer::ChooseCards(CardsChoice(over)))
            .is_err()
    );
    let within = r6003_ids(&driver, &decision, &["Four", "Two"]);
    driver.answer(Answer::ChooseCards(CardsChoice(within)))?;
    let state = driver.state();
    let back: Vec<String> = state
        .zone(Zone::Battlefield)
        .iter()
        .map(|id| name_of(state, *id))
        .filter(|name| ["Two", "Four"].contains(&name.as_str()))
        .collect();
    assert_eq!(back.len(), 2);
    // "Exile this card."
    assert!(
        state
            .zone(Zone::Exile)
            .iter()
            .any(|id| name_of(state, *id) == "Moorland Rescuer")
    );
    Ok(())
}
