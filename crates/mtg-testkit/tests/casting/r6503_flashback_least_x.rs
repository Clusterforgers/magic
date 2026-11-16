//! Goal: a flashback cost that removes X loyalty counters and says "If you cast this spell this way, X can't be 0" (Light Up the Night, CR 601.2b) offers X from 1 to the loyalty among the caster's planeswalkers, removes that many and deals X damage (X plus 1 to a creature or planeswalker), at tables of two to four seats; cast from the hand X may be 0.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use crate::common::casting::lands;
use crate::common::{names, pid};
use mtg_cards::CardDefinition;
use mtg_cards::value::CounterKind;
use mtg_core::card::CardType;
use mtg_engine::choose::NumberChoice;
use mtg_engine::choose::{Target, TargetChoice};
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{island, mountain};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready, with_counters};
use rstest::rstest;

/// A planeswalker with no abilities.
fn walker() -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new("Walker")
            .with_type(CardType::Planeswalker)
            .with_loyalty(1),
    )
}

/// Seat 0 with Light Up the Night in the graveyard (or hand), six Mountains and a planeswalker with `loyalty` counters.
fn table(seats: usize, loyalty: u32, from_hand: bool) -> Scenario {
    let walker = with_counters(ready(walker()), CounterKind::Loyalty, loyalty);
    let mut scenario =
        Scenario::new(seats).battlefield(0, lands(8, mountain).into_iter().chain([walker]));
    scenario = if from_hand {
        scenario.hand(0, [card("Light Up the Night").unwrap()])
    } else {
        scenario.graveyard_named(0, ["Light Up the Night"])
    };
    for seat in 0..seats {
        scenario = scenario.library(seat, (0..10).map(|_| island()));
    }
    scenario
}

/// Casts Light Up the Night at the last seat with X = `x`; the least and most X offered.
fn cast(driver: &mut Driver, seats: usize, x: u32) -> Result<(u32, Option<u32>), ScriptError> {
    driver.begin_cast("Light Up the Night")?;
    let mut offered = (u32::MAX, None);
    for _ in 0..12 {
        let decision = driver.decision()?.clone();
        match &decision.kind {
            DecisionKind::ChooseNumber(options) => {
                offered = (options.min, options.max);
                driver.answer(Answer::ChooseNumber(NumberChoice::Number(x)))?;
            }
            DecisionKind::ChooseTargets(_) => {
                let at = Target::Player(pid(seats - 1));
                driver.answer(Answer::ChooseTargets(TargetChoice::Targets(vec![at])))?;
            }
            DecisionKind::PayMana(_) => {
                driver.answer(Answer::PayMana(PayChoice::Auto))?;
                break;
            }
            other => {
                driver.answer_passively()?;
                assert!(!other.name().is_empty());
            }
        }
    }
    Ok(offered)
}

fn resolve(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..20 {
        if driver.state().stack().is_empty() {
            break;
        }
        driver.answer_passively()?;
    }
    Ok(())
}

#[rstest]
#[case::two_seats_one(2, 3, 1)]
#[case::three_seats_all(3, 3, 3)]
#[case::four_seats_two_of_four(4, 4, 2)]
fn flashback_offers_x_from_one(
    #[case] seats: usize,
    #[case] loyalty: u32,
    #[case] x: u32,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table(seats, loyalty, false))?;
    let before = driver.state().player(pid(seats - 1)).unwrap().life();
    let offered = cast(&mut driver, seats, x)?;
    assert_eq!(
        offered,
        (1, Some(loyalty)),
        "X can't be 0 and is held to the loyalty"
    );
    resolve(&mut driver)?;
    let life = driver.state().player(pid(seats - 1)).unwrap().life();
    assert_eq!(before - life, i32::try_from(x).unwrap());
    assert_eq!(names(driver.state(), Zone::Exile), ["Light Up the Night"]);
    let left = loyalty - x;
    let walkers = names(driver.state(), Zone::Battlefield)
        .into_iter()
        .filter(|name| name == "Walker")
        .count();
    assert_eq!(
        walkers,
        usize::from(left > 0),
        "a walker left with no loyalty dies"
    );
    Ok(())
}

#[test]
fn cast_from_the_hand_x_may_be_zero() -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table(2, 3, true))?;
    let (least, _) = cast(&mut driver, 2, 0)?;
    assert_eq!(least, 0);
    resolve(&mut driver)?;
    assert_eq!(
        names(driver.state(), Zone::Graveyard(pid(0))),
        ["Light Up the Night"]
    );
    Ok(())
}
