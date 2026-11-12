//! Goal: Suppression Ray taps every creature of the target player, lets its controller pay any amount of energy up to what they hold, and puts a stun counter on that many of the creatures it tapped (never on one that was tapped before); a stun counter keeps a permanent tapped through one untap step and goes in its place, whichever the amounts; and the log replays to the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;
use crate::dk1f_cards::{assert_replays, first_main};

use common::pid;
use mtg_cards::value::CounterKind;
use mtg_engine::choose::{CardsChoice, NumberChoice, YesNoChoice};
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::turn::Step;
use mtg_testkit::cards::{elf, giant, plains};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready, tapped, with_counters};
use rstest::rstest;

const RAY: &str = "Suppression Ray // Orderly Plaza";
const BEARS: [&str; 3] = ["Grizzly Bears", "Llanowar Elves", "Serra Angel"];

fn energy() -> CounterKind {
    CounterKind::Other("energy".into())
}

fn stun() -> CounterKind {
    CounterKind::Other("stun".into())
}

/// Seat 0 holds the Ray, five Plains and `held` energy; seat 1 controls three untapped creatures and one that is tapped.
fn table(seats: usize, held: u32) -> Scenario {
    let mut creatures: Vec<_> = BEARS
        .iter()
        .map(|name| ready(card(name).unwrap()))
        .collect();
    creatures.push(tapped(giant()));
    let mut table = first_main(seats)
        .hand_named(0, [RAY])
        .battlefield(0, vec![ready(plains()); 5])
        .counters(0, energy(), held)
        .battlefield(1, creatures);
    for seat in 2..seats {
        table = table.battlefield(seat, [ready(elf())]);
    }
    table
}

/// Resolves the Ray: pays `pay` energy when it is `Some`, and chooses the first creatures that are offered.
fn resolve(driver: &mut Driver, pay: Option<u32>) -> Result<(), ScriptError> {
    for _ in 0..40 {
        if driver.state().stack().is_empty() && driver.state().resolving().is_none() {
            return Ok(());
        }
        match &driver.decision()?.kind {
            DecisionKind::YesNo(_) => {
                driver.answer(Answer::YesNo(YesNoChoice(pay.is_some())))?;
            }
            DecisionKind::ChooseNumber(options) => {
                let x = pay.unwrap_or(0);
                assert!(x <= options.max.unwrap_or(0), "{x} is within {options:?}");
                driver.answer(Answer::ChooseNumber(NumberChoice::Number(x)))?;
            }
            DecisionKind::ChooseCards(options) => {
                let chosen = options.from[..options.max as usize].to_vec();
                driver.answer(Answer::ChooseCards(CardsChoice(chosen)))?;
            }
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    Err(ScriptError::Stuck("the Ray never finished".into()))
}

fn stunned(driver: &Driver, name: &str) -> u32 {
    let id = driver.id_of(name).unwrap();
    driver
        .state()
        .object(id)
        .and_then(|o| o.permanent())
        .map_or(0, |p| p.counters_of(&stun()))
}

fn is_tapped(driver: &Driver, name: &str) -> bool {
    let id = driver.id_of(name).unwrap();
    driver
        .state()
        .object(id)
        .and_then(|o| o.permanent())
        .is_some_and(mtg_engine::object::PermanentState::tapped)
}

#[rstest]
fn the_ray_stuns_as_many_of_the_creatures_it_tapped_as_the_energy_paid(
    #[values(2, 3)] seats: usize,
    #[values(None, Some(0), Some(1), Some(2), Some(3), Some(4))] pay: Option<u32>,
) -> Result<(), ScriptError> {
    let held = 4;
    let setup = table(seats, held).setup()?;
    let mut driver = Driver::from_scenario(table(seats, held))?;
    let target = driver.target_player(1);
    driver.cast_at("Suppression Ray", &[target])?;
    resolve(&mut driver, pay)?;
    for name in BEARS.iter().chain(&["Craw Wurm"]) {
        assert!(is_tapped(&driver, name), "{name} is tapped");
    }
    let paid = pay.unwrap_or(0);
    let counted: u32 = BEARS.iter().map(|name| stunned(&driver, name)).sum();
    // Only the three the Ray tapped can be stunned: the Giant was tapped already.
    assert_eq!(counted, paid.min(3), "{pay:?}");
    assert_eq!(stunned(&driver, "Craw Wurm"), 0);
    let left = driver
        .state()
        .player(pid(0))
        .unwrap()
        .counters_of(&energy());
    assert_eq!(left, held - paid);
    // The creatures of other seats are left alone.
    if seats > 2 {
        assert!(!is_tapped(&driver, "Llanowar Elf"));
    }
    assert_replays(&driver, &setup)
}

#[rstest]
fn the_ray_offers_no_more_energy_than_the_player_holds(
    #[values(0, 1, 3)] held: u32,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table(2, held))?;
    let target = driver.target_player(1);
    driver.cast_at("Suppression Ray", &[target])?;
    while !matches!(driver.decision()?.kind, DecisionKind::YesNo(_)) {
        driver.pass()?;
    }
    driver.answer(Answer::YesNo(YesNoChoice(true)))?;
    let DecisionKind::ChooseNumber(options) = &driver.decision()?.kind else {
        panic!("a number was expected");
    };
    assert_eq!((options.min, options.max), (0, Some(held)));
    Ok(())
}

#[rstest]
fn a_stun_counter_keeps_a_permanent_tapped_through_one_untap_step_each(
    #[values(0, 1, 2, 3)] counters: u32,
    #[values(1, 2, 3, 4)] untaps: u32,
    #[values(2, 3)] seats: usize,
) -> Result<(), ScriptError> {
    let seats_u32 = u32::try_from(seats).unwrap();
    let bears = tapped(card("Grizzly Bears").unwrap());
    let bears = if counters == 0 {
        bears
    } else {
        with_counters(bears, stun(), counters)
    };
    let table = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .battlefield(0, [bears, tapped(giant())]);
    let setup = table.clone().setup()?;
    let mut driver = Driver::from_scenario(table)?;
    // Seat 0's untap step comes at the start of its next turns: 1 + seats, 1 + 2 seats, ...
    driver.idle_until(1 + seats_u32 * untaps, Step::Main1)?;
    let left = counters.saturating_sub(untaps);
    assert_eq!(
        stunned(&driver, "Grizzly Bears"),
        left,
        "{counters} {untaps}"
    );
    assert_eq!(is_tapped(&driver, "Grizzly Bears"), counters >= untaps);
    assert!(!is_tapped(&driver, "Craw Wurm"));
    assert_replays(&driver, &setup)
}
