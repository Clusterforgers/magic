//! Goal: "Bloodthirst X" puts as many +1/+1 counters on the creature as the damage dealt to its controller's opponents this turn, added up over every opponent, and none when only its controller was dealt damage; at 2 to 4 seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_cards::value::CounterKind;
use mtg_core::color::Color;
use mtg_testkit::cards::land;
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// Passes until the stack is empty.
fn settle(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..16 {
        if driver.state().stack().is_empty() {
            return Ok(());
        }
        driver.pass()?;
    }
    Ok(())
}

/// Seat 0 Bolts each seat of `bolted` in turn, then casts Petrified Wood-Kin; the +1/+1 counters it enters with.
fn wood_kin_after(seats: usize, bolted: &[usize]) -> Result<u32, ScriptError> {
    let mut lands: Vec<_> = (0..7).map(|_| ready(land(Color::Green))).collect();
    lands.extend((0..bolted.len()).map(|_| ready(land(Color::Red))));
    let mut hand = vec![card("Petrified Wood-Kin").unwrap()];
    hand.extend(bolted.iter().map(|_| card("Lightning Bolt").unwrap()));
    let mut driver =
        Driver::from_scenario(Scenario::new(seats).hand(0, hand).battlefield(0, lands))?;
    for seat in bolted {
        let target = driver.target_player(*seat);
        driver.cast_at("Lightning Bolt", &[target])?;
        settle(&mut driver)?;
    }
    driver.cast("Petrified Wood-Kin")?;
    settle(&mut driver)?;
    let id = driver.id_of("Petrified Wood-Kin")?;
    let permanent = driver
        .state()
        .object(id)
        .and_then(|object| object.permanent())
        .unwrap();
    Ok(permanent.counters_of(&CounterKind::PlusOnePlusOne))
}

#[rstest]
fn the_damage_to_every_opponent_is_added_up(
    #[values(3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    assert_eq!(wood_kin_after(seats, &[1, 2])?, 6);
    Ok(())
}

#[rstest]
#[case::one_opponent_bolted(&[1], 3)]
#[case::the_same_opponent_twice(&[1, 1], 6)]
#[case::no_damage(&[], 0)]
#[case::only_the_controller_was_dealt_damage(&[0], 0)]
fn bloodthirst_x_counts_the_damage_dealt_to_opponents(
    #[values(2, 3, 4)] seats: usize,
    #[case] bolted: &[usize],
    #[case] counters: u32,
) -> Result<(), ScriptError> {
    assert_eq!(wood_kin_after(seats, bolted)?, counters);
    Ok(())
}
