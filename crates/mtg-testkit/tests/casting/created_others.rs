//! Goal: "Create X tokens. If X is 5 or more, destroy all other creatures." spares the tokens the spell created (Martial
//! Coup and White Sun's Twilight rulings): for X below 5 every creature stays, from 5 on every creature but the new tokens
//! dies, the caster's and each opponent's alike, at every table size, and the log replays to the same state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use common::names;
use common::removal::{creatures, table};
use mtg_engine::choose::NumberChoice;
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::zones::Zone;
use mtg_engine::{Engine, replay};
use mtg_testkit::cards::plains;
use mtg_testkit::{Driver, ScriptError};
use rstest::rstest;

/// Casts `spell` from seat 0 announcing `x` and resolves it; returns the driver and the setup it started from.
fn cast_with_x(spell: &str, seats: usize, x: u32) -> Result<(Driver, mtg_engine::GameSetup), ScriptError> {
    let lands: Vec<_> = (0..x + 2).map(|_| plains()).collect();
    let scenario = table(seats, &[spell], &lands, &[]);
    let setup = scenario.clone().setup()?;
    let mut driver = Driver::from_scenario(scenario)?;
    driver.begin_cast(spell)?;
    if matches!(driver.decision()?.kind, DecisionKind::ChooseNumber(_)) {
        driver.answer(Answer::ChooseNumber(NumberChoice::Number(x)))?;
    }
    if matches!(driver.decision()?.kind, DecisionKind::PayMana(_)) {
        driver.answer(Answer::PayMana(PayChoice::Auto))?;
    }
    while !driver.state().stack().is_empty() || driver.state().resolving().is_some() {
        driver.answer_passively()?;
    }
    Ok((driver, setup))
}

#[rstest]
#[case::martial_coup("Martial Coup", "Soldier")]
#[case::white_suns_twilight("White Sun's Twilight", "Phyrexian Mite")]
fn the_created_tokens_survive_and_the_other_creatures_die_from_x_five(
    #[case] spell: &str,
    #[case] token: &str,
    #[values(4, 5, 7)] x: u32,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let (driver, setup) = cast_with_x(spell, seats, x)?;
    let battlefield = names(driver.state(), Zone::Battlefield);
    let made = battlefield.iter().filter(|name| *name == token).count();
    assert_eq!(made, x as usize, "every token {spell} created survives");
    for (seat, board) in creatures(seats).iter().enumerate() {
        for creature in board {
            let alive = battlefield.contains(&creature.name);
            assert_eq!(alive, x < 5, "{} of seat {seat} at X = {x}", creature.name);
        }
    }
    let replayed = replay(&setup, driver.engine().events())?;
    assert_eq!(replayed.state_hash(), driver.state().state_hash(), "replay");
    let resumed = Engine::resume(setup, driver.engine().events().to_vec())?;
    assert_eq!(resumed.state_hash(), driver.state().state_hash(), "resume");
    Ok(())
}
