//! Goal: a permission to play a limited number of exiled cards counts its uses over every turn it lasts: March of
//! Reckless Joy ("You may play up to two of those cards until the end of your next turn") lets two of the exiled cards be
//! played, both this turn or one now and one on the next turn, never a third; each log replays; at two to four seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_engine::choose::{NumberChoice, YesNoChoice};
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_engine::{GameSetup, replay};
use mtg_testkit::cards::{free_creature, mountain};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError};
use rstest::rstest;

use crate::common::casting::lands;
use crate::common::{names, pid};

/// Answers until the stack is empty and a player has priority: X with `x`, no to every yes/no, payments automatically.
fn settle(driver: &mut Driver, x: u32) -> Result<(), ScriptError> {
    for _ in 0..96 {
        let decision = driver.decision()?.clone();
        let answer = match &decision.kind {
            DecisionKind::Priority(_) if driver.state().zone(Zone::Stack).is_empty() => {
                return Ok(());
            }
            DecisionKind::ChooseNumber(_) => Answer::ChooseNumber(NumberChoice::Number(x)),
            DecisionKind::YesNo(_) => Answer::YesNo(YesNoChoice(false)),
            DecisionKind::PayMana(_) => Answer::PayMana(PayChoice::Auto),
            _ => Driver::passive_answer(&decision),
        };
        driver.answer(answer)?;
    }
    Err(ScriptError::Stuck("the stack does not empty".into()))
}

/// Seat 0 holds March of Reckless Joy with four Mountains; the top of its library is three free creatures.
fn table(seats: usize) -> Scenario {
    let mut scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .hand(0, [card("March of Reckless Joy").unwrap()])
        .battlefield(0, lands(4, mountain))
        .library(
            0,
            [
                free_creature("Ace", 1, 1),
                free_creature("Deuce", 1, 1),
                free_creature("Trey", 1, 1),
                free_creature("Filler", 1, 1),
                free_creature("Filler", 1, 1),
            ],
        );
    for seat in 1..seats {
        scenario = scenario.library(seat, (0..4).map(|_| free_creature("Filler", 1, 1)));
    }
    scenario
}

fn replays(driver: &Driver, setup: &GameSetup) {
    let replayed = replay(setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
}

/// Casts `name` from exile; whether the cast was allowed.
fn cast_from_exile(driver: &mut Driver, name: &str) -> Result<bool, ScriptError> {
    if driver.begin_cast(name).is_err() {
        return Ok(false);
    }
    settle(driver, 0)?;
    Ok(true)
}

#[rstest]
#[case::two_now(&["Ace", "Deuce"], &[])]
#[case::one_now_one_next_turn(&["Ace"], &["Deuce"])]
#[case::none_now_two_next_turn(&[], &["Ace", "Deuce"])]
fn p158_up_to_two_of_the_exiled_cards_are_played(
    #[values(2, 3, 4)] seats: usize,
    #[case] now: &[&str],
    #[case] next: &[&str],
) -> Result<(), ScriptError> {
    let setup = table(seats).setup().unwrap();
    let mut driver = Driver::from_scenario(table(seats))?;
    driver.begin_cast("March of Reckless Joy")?;
    settle(&mut driver, 3)?;
    let exiled = names(driver.state(), Zone::Exile);
    assert_eq!(exiled.len(), 3, "{exiled:?}");
    for name in now {
        assert!(cast_from_exile(&mut driver, name)?, "{name} now");
    }
    if next.is_empty() {
        assert!(!cast_from_exile(&mut driver, "Trey")?, "no third card");
        replays(&driver, &setup);
        return Ok(());
    }
    let turn = u32::try_from(seats).unwrap() + 1;
    driver.idle_until(turn, Step::Main1)?;
    settle(&mut driver, 0)?;
    for name in next {
        assert!(cast_from_exile(&mut driver, name)?, "{name} next turn");
    }
    assert!(!cast_from_exile(&mut driver, "Trey")?, "no third card");
    let field = names(driver.state(), Zone::Battlefield);
    assert!(field.contains(&"Ace".to_owned()) && field.contains(&"Deuce".to_owned()));
    assert_eq!(
        names(driver.state(), Zone::Graveyard(pid(0))),
        ["March of Reckless Joy"]
    );
    replays(&driver, &setup);
    Ok(())
}
