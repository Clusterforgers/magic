//! Goal: "Ninjutsu abilities you activate cost {1} less to activate." takes {1} off the ninjutsu of its controller's cards and leaves another player's ninjutsu and other activated abilities alone; at 2 to 4 seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common::pid;
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::turn::Step;
use mtg_testkit::cards::{forest, island};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// Seat 0 holds Ninja of the Deep Hours ({1}{U} ninjutsu) and has Grizzly Bears and two Islands; seat `master` controls
/// a Silver-Fur Master.
fn scenario(seats: usize, master: usize) -> Scenario {
    let mut mine = vec![
        ready(island()),
        ready(island()),
        ready(card("Grizzly Bears").unwrap()),
    ];
    let mut scenario = Scenario::new(seats).hand_named(0, ["Ninja of the Deep Hours"]);
    let silver = ready(card("Silver-Fur Master").unwrap());
    if master == 0 {
        mine.push(silver);
    } else {
        scenario = scenario.battlefield(master, [silver]);
    }
    (0..seats).fold(scenario.battlefield(0, mine), |scenario, seat| {
        scenario.library(seat, (0..8).map(|_| forest()))
    })
}

/// Seat 0 attacks seat 1 with Grizzly Bears, nobody blocks, and the ninjutsu of the Ninja is begun; the mana asked.
fn ninjutsu_cost(driver: &mut Driver) -> Result<String, ScriptError> {
    driver.idle_until(1, Step::DeclareAttackers)?;
    driver.attack(&[("Grizzly Bears", 1)])?;
    for _ in 0..40 {
        if driver.state().turn().step() == Step::DeclareBlockers
            && matches!(driver.decision()?.kind, DecisionKind::Priority(_))
            && driver.decision()?.player == pid(0)
        {
            break;
        }
        if matches!(driver.decision()?.kind, DecisionKind::DeclareBlockers(_)) {
            driver.answer(Answer::DeclareBlockers(Vec::new()))?;
        } else {
            driver.answer_passively()?;
        }
    }
    driver.begin_activation("Ninja of the Deep Hours")?;
    for _ in 0..4 {
        match &driver.decision()?.kind {
            DecisionKind::ChooseCards(options) => {
                let pick = options.from.clone();
                driver.answer(Answer::ChooseCards(mtg_engine::choose::CardsChoice(pick)))?;
            }
            DecisionKind::PayMana(options) => return Ok(options.cost.to_string()),
            other => panic!("unexpected {}", other.name()),
        }
    }
    panic!("no payment was asked")
}

#[rstest]
#[case::your_master_takes_one_off(0, "{U}")]
#[case::an_opponents_master_does_not(1, "{1}{U}")]
fn silver_fur_master_reduces_its_controllers_ninjutsu(
    #[values(2, 3, 4)] seats: usize,
    #[case] master: usize,
    #[case] cost: &str,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(scenario(seats, master))?;
    assert_eq!(ninjutsu_cost(&mut driver)?, cost);
    Ok(())
}
