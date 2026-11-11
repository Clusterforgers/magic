//! Goal: Mana Vapors, Misstep and Exhaustion hold down the permanents of the kinds they name that the target player controls through that player's next untap step only, leave the other kinds and every other player's permanents to untap, at tables of two, three and four seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use common::pid;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{free_creature, island, mountain};
use mtg_testkit::{Driver, Scenario, ScriptError, ready, tapped};
use rstest::rstest;

/// Whether each permanent named `name` that seat controls is tapped.
fn tapped_of(driver: &Driver, seat: usize, name: &str) -> Vec<bool> {
    let state = driver.state();
    state
        .zone(Zone::Battlefield)
        .iter()
        .filter_map(|id| state.object(*id))
        .filter(|o| o.card().name == name && o.controller() == pid(seat))
        .map(|o| {
            o.permanent()
                .is_some_and(mtg_engine::object::PermanentState::tapped)
        })
        .collect()
}

#[rstest]
#[case::vapors_hold_lands("Mana Vapors", true, false)]
#[case::misstep_holds_creatures("Misstep", false, true)]
#[case::exhaustion_holds_both("Exhaustion", true, true)]
fn the_target_players_permanents_skip_their_next_untap_step(
    #[case] card: &str,
    #[case] lands: bool,
    #[case] creatures: bool,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut scenario = Scenario::new(seats)
        .hand_named(0, [card])
        .battlefield(0, [ready(island()), ready(island()), ready(island())]);
    for seat in 1..seats {
        scenario = scenario.battlefield(
            seat,
            [tapped(mountain()), tapped(free_creature("Bear", 2, 2))],
        );
    }
    let mut driver = Driver::from_scenario(scenario)?;
    let target = driver.target_player(1);
    driver.cast_at(card, &[target])?;
    while !driver.state().stack().is_empty() {
        driver.pass()?;
    }
    // Seat 1 takes turn 2; the other seats follow.
    driver.idle_until(2, Step::Main1)?;
    assert_eq!(tapped_of(&driver, 1, "Mountain"), [lands]);
    assert_eq!(tapped_of(&driver, 1, "Bear"), [creatures]);
    let last = u32::try_from(seats).unwrap();
    driver.idle_until(last, Step::Main1)?;
    for seat in 2..seats {
        assert_eq!(tapped_of(&driver, seat, "Mountain"), [false], "seat {seat}");
        assert_eq!(tapped_of(&driver, seat, "Bear"), [false], "seat {seat}");
    }
    // Seat 1's untap step after the next untaps everything.
    driver.idle_until(last + 2, Step::Main1)?;
    assert_eq!(tapped_of(&driver, 1, "Mountain"), [false]);
    assert_eq!(tapped_of(&driver, 1, "Bear"), [false]);
    Ok(())
}
