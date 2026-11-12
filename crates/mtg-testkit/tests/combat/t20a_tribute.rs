//! Goal: the compiled Tribute to the World Tree draws a card for a creature that enters under its controller's control with power 3 or greater and puts two +1/+1 counters on a smaller one, and does nothing for a creature an opponent controls, at two seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common::pid;
use mtg_cards::value::CounterKind;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::free_creature;
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// Seat 0 controls the Tribute and has a `power`/4 creature "Guest" in hand; seat 1 has the same in hand; `active` is
/// the seat whose turn it is.
fn table(active: usize, power: i32) -> Result<Driver, ScriptError> {
    let filler = || (0..12).map(|_| free_creature("Filler", 1, 1));
    Driver::from_scenario(
        Scenario::new(2)
            .at(2, active, Step::Main1)
            .library(0, filler())
            .library(1, filler())
            .battlefield(0, [ready(card("Tribute to the World Tree").unwrap())])
            .hand(active, [free_creature("Guest", power, 4)]),
    )
}

fn counters(driver: &Driver) -> u32 {
    let id = driver.id_of("Guest").unwrap();
    driver
        .state()
        .object(id)
        .and_then(|object| object.permanent())
        .map_or(0, |permanent| {
            permanent.counters_of(&CounterKind::PlusOnePlusOne)
        })
}

#[rstest]
#[case::power_three_draws(0, 3, 1, 0)]
#[case::power_five_draws(0, 5, 1, 0)]
#[case::power_two_grows(0, 2, 0, 2)]
#[case::power_zero_grows(0, 0, 0, 2)]
#[case::an_opponents_creature_does_nothing(1, 3, 0, 0)]
fn a_creature_entering_draws_or_grows_by_its_power(
    #[case] seat: usize,
    #[case] power: i32,
    #[case] drawn: usize,
    #[case] grown: u32,
) -> Result<(), ScriptError> {
    let mut driver = table(seat, power)?;
    let hand = |driver: &Driver| driver.state().zone(Zone::Hand(pid(0))).len();
    driver.cast("Guest")?;
    for _ in 0..20 {
        if driver.state().stack().is_empty() && driver.state().resolving().is_none() {
            break;
        }
        driver.answer_passively()?;
    }
    // Seat 0 started with an empty hand (or cast its only card), so what it holds now was drawn.
    assert_eq!(hand(&driver), drawn, "cards drawn");
    assert_eq!(counters(&driver), grown, "counters");
    Ok(())
}
