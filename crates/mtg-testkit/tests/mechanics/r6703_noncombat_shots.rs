//! Goal: `TriggerPattern::DealsToughnessDamage` triggers exactly when a source the selector picks deals damage to a
//! creature equal to that creature's toughness (more or less does not), and a damage shield of "plus X" fixes X as it
//! begins and adds it to each noncombat damage event of the sources it covers this turn, to players and permanents:
//! Taii Wakeen, Perfect Shot, at every table size.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_engine::choose::NumberChoice;
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{free_creature, pinger, wastes};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::pid;

fn settle(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..200 {
        if driver.state().stack().is_empty()
            && driver.state().resolving().is_none()
            && driver.state().pending_triggers().is_empty()
        {
            return Ok(());
        }
        driver.answer_passively()?;
    }
    Err(ScriptError::Stuck("the stack".into()))
}

/// Seat 0 has Taii, the Pinger, four Wastes and a library; seat 1 a 1/1 Mouse and a 3/3 Bear.
fn table(seats: usize) -> Result<Driver, ScriptError> {
    let lands: Vec<_> = (0..4).map(|_| ready(wastes())).collect();
    let library: Vec<_> = (0..5).map(|_| wastes()).collect();
    let mut scenario = Scenario::new(seats)
        .battlefield(
            0,
            [
                ready(card("Taii Wakeen, Perfect Shot").unwrap()),
                ready(pinger("Pinger", 0)),
            ]
            .into_iter()
            .chain(lands),
        )
        .library(0, library)
        .battlefield(1, [ready(free_creature("Mouse", 1, 1)), ready(free_creature("Bear", 3, 3))]);
    for seat in 2..seats {
        scenario = scenario.battlefield(seat, [ready(free_creature("Bystander", 1, 1))]);
    }
    Driver::from_scenario(scenario)
}

/// The Pinger deals its damage to `target` (a permanent's name, or seat 1 when `None`).
fn ping(driver: &mut Driver, target: Option<&str>) -> Result<(), ScriptError> {
    driver.begin_activation("Pinger")?;
    let target = match target {
        Some(name) => driver.target_named(name)?,
        None => driver.target_player(1),
    };
    driver.choose_targets(&[target])?;
    settle(driver)
}

/// Taii's ability for X.
fn shield(driver: &mut Driver, x: u32) -> Result<(), ScriptError> {
    driver.begin_activation("Taii Wakeen, Perfect Shot")?;
    if matches!(driver.decision()?.kind, DecisionKind::ChooseNumber(_)) {
        driver.answer(Answer::ChooseNumber(NumberChoice::Number(x)))?;
    }
    if matches!(driver.decision()?.kind, DecisionKind::PayMana(_)) {
        driver.answer(Answer::PayMana(PayChoice::Auto))?;
    }
    settle(driver)
}

fn hand(driver: &Driver) -> usize {
    driver.state().zone(Zone::Hand(pid(0))).len()
}

#[rstest]
#[case::equal_to_toughness("Mouse", 0, 1)]
#[case::less_than_toughness("Bear", 0, 0)]
#[case::raised_to_toughness("Bear", 2, 1)]
#[case::raised_past_toughness("Mouse", 1, 0)]
fn damage_equal_to_toughness_draws(
    #[case] target: &str,
    #[case] x: u32,
    #[case] drawn: usize,
    #[values(2, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = table(seats)?;
    if x > 0 {
        shield(&mut driver, x)?;
    }
    ping(&mut driver, Some(target))?;
    assert_eq!(hand(&driver), drawn);
    Ok(())
}

#[rstest]
fn the_shield_adds_x_to_noncombat_damage_to_a_player(
    #[values(0, 1, 3)] x: u32,
    #[values(2, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = table(seats)?;
    shield(&mut driver, x)?;
    ping(&mut driver, None)?;
    let expected = 20 - 1 - i32::try_from(x).unwrap();
    assert_eq!(driver.state().player(pid(1)).unwrap().life(), expected);
    Ok(())
}
