//! Goal: an extra turn with a rider for its end step (`ExtraTurn.at_end`) sets that delayed trigger up only as the
//! extra turn begins, so Chance for Glory's caster loses at the end step of the extra turn and not of the turn it was
//! cast in, and the creatures it made indestructible stay so into that turn (Chance for Glory rulings), at every table
//! size.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_cards::keyword::Keyword;
use mtg_engine::Event;
use mtg_engine::turn::Step;
use mtg_testkit::cards::{free_creature, mountain, plains};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::pid;

const START: u32 = 4;

/// Seat 0 casts Chance for Glory in its first main phase with a Bear in play; every event until turn `until` has
/// begun, or the game ended.
fn chance(seats: usize, until: u32) -> Result<(Driver, Vec<Event>), ScriptError> {
    let mut scenario = Scenario::new(seats)
        .at(START, 0, Step::Main1)
        .hand(0, [card("Chance for Glory").unwrap()])
        .battlefield(
            0,
            [
                ready(mountain()),
                ready(plains()),
                ready(plains()),
                ready(free_creature("Bear", 2, 2)),
            ],
        );
    for seat in 1..seats {
        scenario = scenario.battlefield(seat, [ready(free_creature("Bystander", 1, 1))]);
    }
    let mut driver = Driver::from_scenario(scenario)?;
    let mut log = driver.cast("Chance for Glory")?;
    for _ in 0..4_000 {
        if driver.engine().decision().is_none() || driver.state().turn().number() >= until {
            break;
        }
        log.extend(driver.answer_passively()?);
    }
    Ok((driver, log))
}

/// The turn each `GameLost` of seat 0 came in, read from the log.
fn lost_in(log: &[Event]) -> Option<u32> {
    let mut turn = START;
    for event in log {
        match event {
            Event::TurnBegan { number, .. } => turn = *number,
            Event::PlayerLost { player, .. } if *player == pid(0) => return Some(turn),
            _ => {}
        }
    }
    None
}

#[rstest]
fn the_caster_loses_at_the_end_step_of_the_extra_turn(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let (driver, log) = chance(seats, START + 3)?;
    let extra = log.iter().find_map(|event| match event {
        Event::TurnBegan {
            number,
            active,
            extra: true,
        } if *active == pid(0) => Some(*number),
        _ => None,
    });
    assert_eq!(extra, Some(START + 1), "the extra turn follows this one");
    assert_eq!(
        lost_in(&log),
        Some(START + 1),
        "lost in the extra turn, not this one"
    );
    assert!(!driver.state().is_alive(pid(0)));
    Ok(())
}

#[rstest]
fn the_creatures_keep_indestructible_into_the_extra_turn(
    #[values(2, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let (driver, _) = chance(seats, START + 1)?;
    assert_eq!(driver.state().turn().number(), START + 1);
    let bear = driver.id_of("Bear")?;
    assert!(mtg_engine::keywords::has_keyword(
        driver.state(),
        bear,
        &Keyword::Indestructible
    ));
    assert!(
        driver.state().is_alive(pid(0)),
        "alive until that turn's end step"
    );
    Ok(())
}
