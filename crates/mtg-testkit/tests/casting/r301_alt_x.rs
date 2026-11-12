//! Goal: an alternative cost a keyword prints with an `{X}` ("Warp {X}{G}", Broodguard Elite) asks X when the card is cast that way, costs X more for each `{X}` and gives the spell that X, at tables of two to four seats; the printed cost keeps its own `{X}{G}{G}`.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common::casting::{choose_way, lands};
use mtg_cards::value::CounterKind;
use mtg_engine::choose::NumberChoice;
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_testkit::cards::{forest, island};
use mtg_testkit::{Driver, Scenario, ScriptError};
use rstest::rstest;

/// Seat 0 with Broodguard Elite in hand and `forests` Forests.
fn table(seats: usize, forests: usize) -> Scenario {
    let mut scenario = Scenario::new(seats)
        .hand_named(0, ["Broodguard Elite"])
        .battlefield(0, lands(forests, forest));
    for seat in 0..seats {
        scenario = scenario.library(seat, (0..10).map(|_| island()));
    }
    scenario
}

/// Casts Broodguard Elite the way `label` names with X = `x`; the most X offered and the mana asked.
fn cast(driver: &mut Driver, label: &str, x: u32) -> Result<(Option<u32>, u32), ScriptError> {
    driver.begin_cast("Broodguard Elite")?;
    if matches!(driver.decision()?.kind, DecisionKind::ChooseOption(_)) {
        choose_way(driver, label)?;
    }
    let DecisionKind::ChooseNumber(options) = &driver.decision()?.kind else {
        panic!("X is asked: {}", driver.decision()?.kind.name());
    };
    let most = options.max;
    driver.answer(Answer::ChooseNumber(NumberChoice::Number(x)))?;
    let asked = driver.pay_options()?.cost.mana_value();
    driver.answer(Answer::PayMana(PayChoice::Auto))?;
    for _ in 0..20 {
        if driver.state().stack().is_empty() {
            break;
        }
        driver.answer_passively()?;
    }
    Ok((most, asked))
}

fn counters(driver: &Driver) -> u32 {
    let id = driver.id_of("Broodguard Elite").unwrap();
    driver
        .state()
        .object(id)
        .and_then(|object| object.permanent())
        .map_or(0, |permanent| {
            permanent.counters_of(&CounterKind::PlusOnePlusOne)
        })
}

#[rstest]
#[case::two_seats(2, 2)]
#[case::three_seats(3, 3)]
#[case::four_seats(4, 1)]
fn warp_announces_x_and_the_elite_enters_with_it(
    #[case] seats: usize,
    #[case] x: u32,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table(seats, 4))?;
    let (most, asked) = cast(&mut driver, "Warp", x)?;
    // Four Forests: {G} and X of three.
    assert_eq!(most, Some(3));
    assert_eq!(asked, x + 1);
    assert_eq!(counters(&driver), x);
    Ok(())
}

#[test]
fn the_printed_cost_keeps_its_own_x() -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table(2, 4))?;
    let (most, asked) = cast(&mut driver, "Cast for", 2)?;
    assert_eq!(most, Some(2));
    assert_eq!(asked, 4);
    assert_eq!(counters(&driver), 2);
    Ok(())
}
