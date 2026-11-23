//! Goal: Volatile Stormdrake's enters trigger exchanges control of it and the target creature an opponent controls, gives
//! its controller four energy, then sacrifices that creature unless they pay energy equal to its mana value (paid,
//! declined, or more than they hold, when only No is offered); at tables of two to four seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_cards::value::CounterKind;
use mtg_core::color::Color;
use mtg_engine::choose::{TargetChoice, YesNoChoice};
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{creature, forest, island};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::{names, pid};

/// Seat 0 holds Volatile Stormdrake with two Islands; seat 1 controls a creature of mana value `value`.
fn table(seats: usize, value: u32) -> Scenario {
    (0..seats).fold(
        Scenario::new(seats)
            .hand(0, [card("Volatile Stormdrake").unwrap()])
            .battlefield(0, [ready(island()), ready(island())])
            .battlefield(
                1,
                [ready(creature("Beast", value - 1, &[Color::Green], 3, 3))],
            ),
        |scenario, seat| scenario.library(seat, vec![forest(); 10]),
    )
}

/// Casts the Stormdrake, targets the Beast with its trigger and answers `pay` to the energy payment; whether paying was
/// offered.
fn play(driver: &mut Driver, pay: bool) -> Result<bool, ScriptError> {
    driver.begin_cast("Volatile Stormdrake")?;
    let mut asked = false;
    for _ in 0..40 {
        let Ok(decision) = driver.decision() else {
            break;
        };
        match &decision.kind.clone() {
            DecisionKind::PayMana(_) => {
                driver.answer(Answer::PayMana(PayChoice::Auto))?;
            }
            DecisionKind::ChooseTargets(_) => {
                let beast = driver.target_named("Beast")?;
                driver.answer(Answer::ChooseTargets(TargetChoice::Targets(vec![beast])))?;
            }
            DecisionKind::YesNo(options) => {
                // Energy short of the mana value leaves No the only answer.
                asked = !options.only_no;
                driver.answer(Answer::YesNo(YesNoChoice(pay && !options.only_no)))?;
            }
            DecisionKind::Priority(_) if driver.state().stack().is_empty() => break,
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    Ok(asked)
}

fn energy(driver: &Driver) -> u32 {
    driver
        .state()
        .player(pid(0))
        .unwrap()
        .counters_of(&CounterKind::Other("energy".to_owned()))
}

#[rstest]
#[case::paid(2, true, true, 2)]
#[case::declined(2, false, false, 4)]
#[case::all_the_energy(4, true, true, 0)]
#[case::more_than_held(5, true, false, 4)]
fn the_stormdrake_trades_itself_and_keeps_the_creature_only_for_energy(
    #[case] value: u32,
    #[case] pay: bool,
    #[case] kept: bool,
    #[case] left: u32,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table(seats, value))?;
    // The payment is offered only while the energy covers the mana value.
    assert_eq!(play(&mut driver, pay)?, value <= 4);
    let drake = driver.id_of("Volatile Stormdrake")?;
    let controller = |id| mtg_engine::characteristics::controller_of(driver.state(), id);
    assert_eq!(controller(drake), Some(pid(1)));
    assert_eq!(energy(&driver), left);
    if kept {
        let beast = driver.id_of("Beast")?;
        assert_eq!(controller(beast), Some(pid(0)));
    } else {
        assert!(names(driver.state(), Zone::Graveyard(pid(1))).contains(&"Beast".to_owned()));
    }
    Ok(())
}
