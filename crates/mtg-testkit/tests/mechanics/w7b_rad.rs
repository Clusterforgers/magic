//! Goal: rad counters (CR 725) mill at the beginning of each player's own precombat main phase, at tables of two to four seats: a card per rad counter, then 1 life and a rad counter for each nonland card milled; lands cost nothing, a seat waits for its own turn, the log shows the milled cards, the loss and the removal, and it replays to the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use common::copying::{life, replays};
use mtg_cards::value::CounterKind;
use mtg_core::ids::PlayerId;
use mtg_engine::Event;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::{Driver, Scenario, ScriptError};
use rstest::rstest;

fn rad() -> CounterKind {
    CounterKind::Other("rad".to_owned())
}

fn rad_of(driver: &Driver, seat: usize) -> u32 {
    driver
        .state()
        .player(PlayerId::from_index(seat))
        .unwrap()
        .counters_of(&rad())
}

fn graveyard(driver: &Driver, seat: usize) -> usize {
    driver
        .state()
        .zone(Zone::Graveyard(PlayerId::from_index(seat)))
        .len()
}

/// Seat 0 holds `rad` rad counters and every other seat four; each library is a Grizzly Bears to draw, then `top` (a
/// Forest for 'L', Grizzly Bears otherwise, the first on top) and ten more Grizzly Bears. The game starts in seat 0's
/// upkeep of turn 2, so every seat draws before it mills.
fn scenario(seats: usize, rad: u32, top: &str) -> Scenario {
    let library: Vec<&str> = std::iter::once("Grizzly Bears")
        .chain(
            top.chars()
                .map(|c| if c == 'L' { "Forest" } else { "Grizzly Bears" }),
        )
        .chain(std::iter::repeat_n("Grizzly Bears", 10))
        .collect();
    let scenario = (0..seats).fold(
        Scenario::new(seats).at(2, 0, Step::Upkeep),
        |scenario, seat| scenario.library_named(seat, library.iter().copied()),
    );
    let scenario = (1..seats).fold(scenario, |scenario, seat| {
        scenario.counters(seat, self::rad(), 4)
    });
    if rad == 0 {
        scenario
    } else {
        scenario.counters(0, self::rad(), rad)
    }
}

#[rstest]
#[case::all_nonland(3, "CCC", 3, 3)]
#[case::lands_cost_nothing(3, "LCL", 3, 1)]
#[case::only_lands(2, "LL", 2, 0)]
fn the_active_player_mills_as_the_precombat_main_phase_begins(
    #[values(2, 3, 4)] seats: usize,
    #[case] rad: u32,
    #[case] top: &str,
    #[case] milled: usize,
    #[case] nonland: u32,
) -> Result<(), ScriptError> {
    let scenario = || scenario(seats, rad, top);
    let setup = scenario().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario())?;
    driver.idle_until(2, Step::Main1)?;
    assert_eq!(graveyard(&driver, 0), milled, "a card per rad counter");
    assert_eq!(life(&driver, 0), 20 - i32::try_from(nonland).unwrap());
    assert_eq!(rad_of(&driver, 0), rad - nonland);
    for seat in 1..seats {
        assert_eq!(
            graveyard(&driver, seat),
            0,
            "seat {seat} waits for its own turn"
        );
        assert_eq!(rad_of(&driver, seat), 4);
    }
    let logged = driver
        .engine()
        .events()
        .iter()
        .any(|e| matches!(e, Event::LifeLost { amount, .. } if *amount == nonland));
    assert_eq!(logged, nonland > 0, "the loss is in the log");
    replays(&driver, &setup);
    Ok(())
}

#[rstest]
fn each_seat_mills_on_its_own_turn(#[values(2, 3, 4)] seats: usize) -> Result<(), ScriptError> {
    let scenario = || scenario(seats, 0, "");
    let setup = scenario().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario())?;
    let last = u32::try_from(seats).unwrap() + 1;
    driver.idle_until(last, Step::BeginCombat)?;
    for seat in 1..seats {
        assert_eq!(graveyard(&driver, seat), 4, "seat {seat} milled four Bears");
        assert_eq!(rad_of(&driver, seat), 0);
        assert_eq!(life(&driver, seat), 16);
    }
    assert_eq!(graveyard(&driver, 0), 0, "no rad counters, nothing milled");
    replays(&driver, &setup);
    Ok(())
}
