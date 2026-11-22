//! Goal: "whenever you face a dilemma" triggers once as its controller chooses one or more modes for a spell they cast, never for a spell without modes, and its effect reads each opponent at 2 to 4 seats; the log replays to the same state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_engine::choose::ModeChoice;
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::{PresetPermanent, replay};
use mtg_testkit::cards::{forest, mountain, plains};
use mtg_testkit::corpus;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::casting::{lands, pay};
use crate::common::pid;

fn card(name: &str) -> Arc<CardDefinition> {
    corpus::card(name).unwrap()
}

/// Seat 0 controls Obsidian Order Operative, a Plains and two Mountains and holds `spell`.
fn game(seats: usize, spell: &str) -> Result<Driver, ScriptError> {
    let battlefield: Vec<PresetPermanent> =
        std::iter::once(ready(card("Obsidian Order Operative")))
            .chain(lands(1, plains))
            .chain(lands(2, mountain))
            .collect();
    let scenario = (0..seats)
        .fold(Scenario::new(seats), |scenario, seat| {
            scenario.library(seat, (0..8).map(|_| forest()))
        })
        .hand_named(0, vec![spell])
        .battlefield(0, battlefield);
    Driver::from_scenario(scenario)
}

/// Casts `spell` with the modes `modes`, aiming any target at seat 1, and lets the stack resolve.
fn cast(driver: &mut Driver, spell: &str, modes: &[usize]) -> Result<(), ScriptError> {
    driver.begin_cast(spell)?;
    for _ in 0..16 {
        match driver.decision()?.kind.clone() {
            DecisionKind::ChooseMode(_) => {
                driver.answer(Answer::ChooseMode(ModeChoice::Modes(modes.to_vec())))?;
            }
            DecisionKind::ChooseTargets(_) => {
                let foe = driver.target_player(1);
                driver.choose_targets(&[foe])?;
            }
            DecisionKind::PayMana(_) => {
                pay(driver)?;
            }
            DecisionKind::Priority(_) if driver.state().stack().is_empty() => return Ok(()),
            DecisionKind::Priority(_) => {
                driver.pass()?;
            }
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    Ok(())
}

fn life(driver: &Driver, seat: usize) -> i32 {
    driver.state().player(pid(seat)).unwrap().life()
}

#[rstest]
// Boros Charm's indestructible mode: one dilemma, each opponent loses 1 and its controller gains 1.
#[case::a_modal_spell("Boros Charm", &[1], 1)]
// Lightning Bolt chooses no mode: no dilemma.
#[case::no_modes("Lightning Bolt", &[], 0)]
fn r116_a_dilemma_triggers_once_for_the_modes_chosen(
    #[values(2, 3, 4)] seats: usize,
    #[case] spell: &str,
    #[case] modes: &[usize],
    #[case] dilemmas: i32,
) -> Result<(), ScriptError> {
    let mut driver = game(seats, spell)?;
    cast(&mut driver, spell, modes)?;
    assert_eq!(life(&driver, 0), 20 + dilemmas);
    // Lightning Bolt aimed at seat 1 takes 3 more there.
    let bolt = if modes.is_empty() { 3 } else { 0 };
    assert_eq!(life(&driver, 1), 20 - dilemmas - bolt);
    for seat in 2..seats {
        assert_eq!(life(&driver, seat), 20 - dilemmas);
    }
    let replayed = replay(driver.engine().setup(), driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
    Ok(())
}
