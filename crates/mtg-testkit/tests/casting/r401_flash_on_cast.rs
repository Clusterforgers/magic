//! Goal: a card that may be cast as though it had flash if a condition on the spell holds ("if X is 3 or less", "if it
//! targets a permanent you control") begins its cast on another player's turn, goes on to its payment when the announced
//! X or target meets the condition and is rewound to its owner's hand when it does not, and on its owner's main phase is
//! cast whatever was announced; at tables of two, three and four seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_core::ids::PlayerId;
use mtg_engine::choose::{NumberChoice, TargetChoice};
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{forest, island};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::names;

/// Passes until `seat` has priority.
fn r401_priority_to(driver: &mut Driver, seat: usize, seats: usize) -> Result<(), ScriptError> {
    for _ in 0..seats {
        if driver.decision()?.player.index() == seat {
            return Ok(());
        }
        driver.pass()?;
    }
    assert_eq!(
        driver.decision()?.player.index(),
        seat,
        "the seat has priority"
    );
    Ok(())
}

/// A table where the last seat holds `name` and eight Islands, and every seat has a Forest; seat 0 is active, or the
/// holder when `own_turn`.
fn r401_table(seats: usize, name: &str, own_turn: bool) -> (Scenario, usize) {
    let holder = seats - 1;
    let active = if own_turn { holder } else { 0 };
    let mut scenario = Scenario::new(seats)
        .at(1, active, Step::Main1)
        .hand(holder, [card(name).unwrap()]);
    for seat in 0..seats {
        let mut permanents = vec![ready(forest())];
        if seat == holder {
            permanents.extend((0..8).map(|_| ready(island())));
        }
        scenario = scenario.battlefield(seat, permanents);
    }
    (scenario, holder)
}

/// Whether the cast went on to its payment; otherwise the card must be back in its owner's hand.
fn r401_reached_payment(
    driver: &mut Driver,
    holder: usize,
    name: &str,
) -> Result<bool, ScriptError> {
    let paying = matches!(driver.decision()?.kind, DecisionKind::PayMana(_));
    if paying {
        driver.answer(Answer::PayMana(PayChoice::Auto))?;
        assert_eq!(names(driver.state(), Zone::Stack), [name]);
    } else {
        assert_eq!(names(driver.state(), Zone::Hand(PlayerId(holder))), [name]);
    }
    Ok(paying)
}

#[rstest]
#[case::three_on_another_turn(3, false, true)]
#[case::four_on_another_turn(4, false, false)]
#[case::four_on_its_own_main_phase(4, true, true)]
fn r401_silver_scrutiny_is_cast_on_another_turn_only_with_x_at_most_three(
    #[case] x: u32,
    #[case] own_turn: bool,
    #[case] cast: bool,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let (scenario, holder) = r401_table(seats, "Silver Scrutiny", own_turn);
    let mut driver = Driver::from_scenario(scenario)?;
    r401_priority_to(&mut driver, holder, seats)?;
    driver.begin_cast("Silver Scrutiny")?;
    driver.answer(Answer::ChooseNumber(NumberChoice::Number(x)))?;
    assert_eq!(
        r401_reached_payment(&mut driver, holder, "Silver Scrutiny")?,
        cast
    );
    Ok(())
}

#[rstest]
#[case::own_permanent_on_another_turn(true, false, true)]
#[case::foreign_permanent_on_another_turn(false, false, false)]
#[case::foreign_permanent_on_its_own_main_phase(false, true, true)]
fn r401_flash_photography_is_cast_on_another_turn_only_at_your_permanent(
    #[case] own_target: bool,
    #[case] own_turn: bool,
    #[case] cast: bool,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let (scenario, holder) = r401_table(seats, "Flash Photography", own_turn);
    let mut driver = Driver::from_scenario(scenario)?;
    r401_priority_to(&mut driver, holder, seats)?;
    driver.begin_cast("Flash Photography")?;
    let DecisionKind::ChooseTargets(options) = &driver.decision()?.kind else {
        panic!("the copy asks for its target");
    };
    let wanted = if own_target { holder } else { 0 };
    let state = driver.state();
    let target = *options
        .legal
        .iter()
        .find(|target| {
            target.object().is_some_and(|object| {
                state.object(object.id).map(|o| o.controller()) == Some(PlayerId(wanted))
            })
        })
        .expect("a permanent of the seat");
    driver.answer(Answer::ChooseTargets(TargetChoice::Targets(vec![target])))?;
    assert_eq!(
        r401_reached_payment(&mut driver, holder, "Flash Photography")?,
        cast
    );
    Ok(())
}
