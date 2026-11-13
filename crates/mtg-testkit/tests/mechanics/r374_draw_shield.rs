//! Goal: a shield over every draw of a targeted player until end of turn ("Until end of turn, if target player would draw a
//! card, instead that player skips that draw and you draw a card.") replaces each draw of that player alone, a caster who
//! targets themselves draws once for each draw, and the log replays to the same state, at tables of two to four seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_engine::{GameSetup, replay};
use mtg_testkit::cards::{free_creature, island};
use mtg_testkit::{Driver, Scenario, ScriptError, corpus, ready};
use rstest::rstest;

use crate::common::pid;

fn replays(driver: &Driver, setup: &GameSetup) {
    let replayed = replay(setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
}

/// The upkeep of `active`'s turn: seat 0 holds Plagiarize and controls four Islands; every library has ten cards.
fn table(seats: usize, active: usize) -> Scenario {
    let library = || (0..10).map(|i| free_creature(&format!("Pad {i}"), 1, 1));
    (0..seats).fold(
        Scenario::new(seats)
            .at(2, active, Step::Upkeep)
            .hand(0, vec![corpus::card("Plagiarize").unwrap()])
            .battlefield(0, (0..4).map(|_| ready(island()))),
        |scenario, seat| scenario.library(seat, library()),
    )
}

#[rstest]
#[case::another_player(1)]
#[case::the_last_seat(9)]
#[case::the_caster(0)]
fn each_draw_of_the_target_is_the_casters(
    #[case] target: usize,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let target = target.min(seats - 1);
    let setup = table(seats, target).setup().unwrap();
    let mut driver = Driver::from_scenario(table(seats, target))?;
    // The active player passes in their upkeep until seat 0 holds priority.
    while driver.decision()?.player != pid(0) {
        driver.pass()?;
    }
    driver.begin_cast("Plagiarize")?;
    let at = driver.target_player(target);
    driver.choose_targets(&[at])?;
    driver.pay_tapping(&["Island", "Island", "Island", "Island"], 0)?;
    driver.pass_until(Step::Main1)?;
    let state = driver.state();
    // The target's draw step draw is seat 0's: one card, from seat 0's library.
    assert_eq!(state.zone(Zone::Hand(pid(0))).len(), 1);
    assert_eq!(state.zone(Zone::Library(pid(0))).len(), 9);
    for seat in 1..seats {
        assert_eq!(
            state.zone(Zone::Library(pid(seat))).len(),
            10,
            "seat {seat}"
        );
        assert!(state.zone(Zone::Hand(pid(seat))).is_empty(), "seat {seat}");
    }
    replays(&driver, &setup);
    Ok(())
}
