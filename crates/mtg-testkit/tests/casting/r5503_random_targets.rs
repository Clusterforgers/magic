//! Goal: Grip of Chaos reselects the target of every spell or ability with a single target at random at tables of two
//! to four seats: the new target is always a legal one (the old one included), the pick follows the seed and differs
//! between seeds, and a spell without targets does not trigger it.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeSet;

use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{bear, burn, forest, mountain};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::casting::lands;
use crate::common::pid;

/// Answers until the stack is empty and a player has priority.
fn r5503_resolve_all(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..64 {
        let decision = driver.decision()?.clone();
        if matches!(decision.kind, DecisionKind::Priority(_))
            && driver.state().zone(Zone::Stack).is_empty()
        {
            return Ok(());
        }
        let answer = match &decision.kind {
            DecisionKind::PayMana(_) => Answer::PayMana(PayChoice::Auto),
            _ => Driver::passive_answer(&decision),
        };
        driver.answer(answer)?;
    }
    Err(ScriptError::Stuck("the stack does not empty".into()))
}

/// Seat 0 controls Grip of Chaos and casts a 3-damage bolt at seat 1 under `seed`; every seat also has a Grizzly Bears.
/// The result is what was dealt damage: a seat that lost life, or a seat whose bear died.
fn r5503_bolt_lands_on(seats: usize, seed: u64) -> Result<String, ScriptError> {
    let mut permanents = lands(2, mountain);
    permanents.push(ready(card("Grip of Chaos").unwrap()));
    permanents.push(ready(bear()));
    let mut scenario = Scenario::new(seats)
        .seed(seed)
        .battlefield(0, permanents)
        .hand(0, vec![burn("Bolt", 3)]);
    for seat in 1..seats {
        scenario = scenario.battlefield(seat, vec![ready(bear())]);
    }
    let mut driver = Driver::from_scenario(scenario)?;
    let target = driver.target_player(1);
    driver.cast_at("Bolt", &[target])?;
    r5503_resolve_all(&mut driver)?;
    let state = driver.state();
    let mut hit = Vec::new();
    for seat in 0..seats {
        if state.player(pid(seat)).unwrap().life() < 20 {
            hit.push(format!("seat {seat}"));
        }
        if !state.zone(Zone::Graveyard(pid(seat))).is_empty()
            && state
                .zone(Zone::Graveyard(pid(seat)))
                .iter()
                .any(|id| crate::common::name_of(state, *id) == "Grizzly Bears")
        {
            hit.push(format!("bear {seat}"));
        }
    }
    assert_eq!(
        hit.len(),
        1,
        "exactly one legal target is dealt the damage: {hit:?}"
    );
    Ok(hit.remove(0))
}

#[rstest]
#[case::two_seats(2)]
#[case::three_seats(3)]
#[case::four_seats(4)]
fn r5503_grip_reselects_among_every_legal_target(#[case] seats: usize) -> Result<(), ScriptError> {
    let mut seen = BTreeSet::new();
    for seed in 0..16 {
        let first = r5503_bolt_lands_on(seats, seed)?;
        // The same seed picks the same target.
        assert_eq!(r5503_bolt_lands_on(seats, seed)?, first);
        seen.insert(first);
    }
    assert!(seen.len() >= 2, "the pick differs between seeds: {seen:?}");
    Ok(())
}

#[rstest]
#[case::two_seats(2)]
#[case::four_seats(4)]
fn r5503_grip_ignores_a_spell_without_targets(#[case] seats: usize) -> Result<(), ScriptError> {
    let mut permanents = lands(2, forest);
    permanents.push(ready(card("Grip of Chaos").unwrap()));
    let scenario = Scenario::new(seats)
        .battlefield(0, permanents)
        .hand(0, vec![bear()]);
    let mut driver = Driver::from_scenario(scenario)?;
    driver.cast("Grizzly Bears")?;
    assert_eq!(driver.state().zone(Zone::Stack).len(), 1);
    assert_eq!(driver.state().pending_triggers(), []);
    Ok(())
}
