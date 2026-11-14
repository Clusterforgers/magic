//! Goal: while Kaervek's Torch is on the stack, spells that target it cost {2} more to cast, at two to four seats: a
//! Counterspell aimed at the Torch takes four lands, one aimed at another spell beside it two, and Kaervek's Torch deals
//! X damage to its target when it resolves; the log replays to the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_engine::choose::{NumberChoice, Target, TargetChoice};
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::replay;
use mtg_engine::turn::Step;
use mtg_testkit::cards::{island, mountain};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::pid;

const TORCH: &str = "Kaervek's Torch";

/// Casts `name` from the hand of the player holding priority with X = 1 at `target`, paying automatically.
fn r6001_cast(driver: &mut Driver, name: &str, target: Target) -> Result<(), ScriptError> {
    driver.begin_cast(name)?;
    for _ in 0..10 {
        match driver.decision()?.kind.clone() {
            DecisionKind::ChooseNumber(_) => {
                driver.answer(Answer::ChooseNumber(NumberChoice::Number(1)))?;
            }
            DecisionKind::ChooseTargets(_) => {
                driver.answer(Answer::ChooseTargets(TargetChoice::Targets(vec![target])))?;
            }
            DecisionKind::PayMana(_) => {
                driver.answer(Answer::PayMana(PayChoice::Auto))?;
                return Ok(());
            }
            _ => return Err(ScriptError::Stuck(format!("casting {name}"))),
        }
    }
    Err(ScriptError::Stuck(format!("casting {name}")))
}

#[rstest]
fn r6001_spells_targeting_the_torch_cost_two_more(
    #[values(2, 3, 4)] seats: usize,
    #[values(true, false)] at_torch: bool,
) -> Result<(), ScriptError> {
    let scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .hand(0, [card(TORCH).unwrap(), card("Lightning Bolt").unwrap()])
        .battlefield(0, (0..3).map(|_| ready(mountain())))
        .hand(1, [card("Counterspell").unwrap()])
        .battlefield(1, (0..4).map(|_| ready(island())));
    let setup = scenario.clone().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario)?;
    let before = driver.state().player(pid(1)).unwrap().life();
    let opponent = driver.target_player(1);
    r6001_cast(&mut driver, TORCH, opponent)?;
    let torch = driver.state().stack()[0];
    r6001_cast(&mut driver, "Lightning Bolt", opponent)?;
    let bolt = *driver
        .state()
        .stack()
        .iter()
        .find(|id| **id != torch)
        .unwrap();
    // Seat 0 passes; seat 1 is next to act.
    driver.pass()?;
    let aim = if at_torch { torch } else { bolt };
    let aim = Target::Object(driver.state().object(aim).unwrap().reference());
    r6001_cast(&mut driver, "Counterspell", aim)?;
    let tapped = driver
        .state()
        .zone(mtg_engine::zones::Zone::Battlefield)
        .iter()
        .filter_map(|id| driver.state().object(*id))
        .filter(|object| object.controller() == pid(1))
        .filter(|object| {
            object
                .permanent()
                .is_some_and(mtg_engine::object::PermanentState::tapped)
        })
        .count();
    assert_eq!(tapped, if at_torch { 4 } else { 2 });
    for _ in 0..60 {
        if driver.state().stack().is_empty() && driver.state().pending_triggers().is_empty() {
            break;
        }
        driver.answer_passively()?;
    }
    let life = driver.state().player(pid(1)).unwrap().life();
    // The countered spell deals nothing; the other one deals its damage (Torch 1, Bolt 3).
    assert_eq!(before - life, if at_torch { 3 } else { 1 });
    let replayed = replay(&setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
    Ok(())
}
