//! Goal: Gideon, Champion of Justice's +1 puts a loyalty counter on Gideon for each creature the target opponent
//! controls (and not on that opponent), counting only that opponent's creatures; at tables of two to four seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_cards::value::CounterKind;
use mtg_engine::decision::DecisionKind;
use mtg_engine::turn::Step;
use mtg_testkit::cards::bear;
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::pid;

const GIDEON: &str = "Gideon, Champion of Justice";

/// Passes until the stack is empty at a priority decision.
fn settle(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..20 {
        if matches!(driver.decision()?.kind, DecisionKind::Priority(_))
            && driver.state().stack().is_empty()
        {
            return Ok(());
        }
        driver.pass()?;
    }
    Ok(())
}

fn loyalty(driver: &Driver) -> u32 {
    let id = driver.id_of(GIDEON).unwrap();
    driver
        .state()
        .object(id)
        .unwrap()
        .permanent()
        .unwrap()
        .counters_of(&CounterKind::Loyalty)
}

#[rstest]
fn pc1_the_plus_one_counts_the_target_opponents_creatures_onto_gideon(
    #[values(2, 3, 4)] seats: usize,
    #[values(0, 1, 3)] creatures: usize,
) -> Result<(), ScriptError> {
    let target = seats - 1;
    // Every other opponent holds one creature, which the count leaves out.
    let scenario = (1..target).fold(
        Scenario::new(seats)
            .at(1, 0, Step::Main1)
            .battlefield(0, [ready(card(GIDEON).unwrap())])
            .battlefield(target, (0..creatures).map(|_| ready(bear()))),
        |scenario, seat| scenario.battlefield(seat, [ready(bear())]),
    );
    let mut driver = Driver::from_scenario(scenario)?;
    let before = loyalty(&driver);
    driver.begin_ability(GIDEON, 0)?;
    driver.choose_targets(&[driver.target_player(target)])?;
    settle(&mut driver)?;
    let expected = before + 1 + u32::try_from(creatures).unwrap();
    assert_eq!(loyalty(&driver), expected, "Gideon's loyalty");
    for seat in 0..seats {
        assert_eq!(
            driver
                .state()
                .player(pid(seat))
                .unwrap()
                .counters_of(&CounterKind::Loyalty),
            0,
            "seat {seat} holds no loyalty counter"
        );
    }
    Ok(())
}
