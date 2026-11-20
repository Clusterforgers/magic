//! Goal: Swinging Ship's visit adds a combat phase after the first combat phase of the turn (CR 500.8, 505.1a), at tables
//! of two to four seats: the first combat comes first, the added one follows it, at the beginning of that combat the
//! creatures that attacked untap and can attack again, a creature that did not attack stays as it was, and the turn goes
//! on to its second main phase; the log replays to the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_core::attraction::Lights;
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::turn::Step;
use mtg_engine::{GameSetup, replay};
use mtg_testkit::cards::{free_creature, un33_opener};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready, tapped};
use rstest::rstest;

use crate::common::pid;

fn replays(driver: &Driver, setup: &GameSetup) {
    let replayed = replay(setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
}

/// Seat 0 has Swinging Ship (lit at every number) as its Attraction deck, an opener in hand, Raider (ready) and Idler
/// (tapped) on the battlefield; it is seat 0's upkeep.
fn table(seats: usize) -> Scenario {
    Scenario::new(seats)
        .seed(1)
        .at(1, 0, Step::Upkeep)
        .attractions(0, [(card("Swinging Ship").unwrap(), Lights::of(1..=6))])
        .hand(0, [un33_opener("Open One", Some(1))])
        .battlefield(
            0,
            [
                ready(free_creature("Raider", 2, 2)),
                tapped(free_creature("Idler", 1, 1)),
            ],
        )
}

fn is_tapped(driver: &Driver, name: &str) -> bool {
    let id = driver.id_of(name).unwrap();
    driver
        .state()
        .object(id)
        .unwrap()
        .permanent()
        .unwrap()
        .tapped()
}

/// Answers passively until seat 0 is asked to declare attackers or has priority in `stop`.
fn until(driver: &mut Driver, stop: Step) -> Result<(), ScriptError> {
    for _ in 0..400 {
        let decision = driver.decision()?.clone();
        match decision.kind {
            DecisionKind::DeclareAttackers(_) if decision.player == pid(0) => return Ok(()),
            DecisionKind::Priority(_)
                if driver.state().turn().step() == stop
                    && driver.state().stack().is_empty()
                    && driver.state().pending_triggers().is_empty() =>
            {
                return Ok(());
            }
            DecisionKind::PayMana(_) => {
                driver.answer(Answer::PayMana(PayChoice::Auto))?;
            }
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    Err(ScriptError::Stuck("the turn".to_owned()))
}

#[rstest]
fn r110_swinging_ship_adds_a_combat_after_the_first(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let setup = table(seats).setup().unwrap();
    let mut driver = Driver::from_scenario(table(seats))?;
    driver.cast("Open One")?;
    until(&mut driver, Step::Main1)?;
    // The visit happens as the precombat main phase begins.
    until(&mut driver, Step::Main2)?;
    assert_eq!(
        driver.state().turn().combats(),
        1,
        "the first combat comes first"
    );
    driver.attack(&[("Raider", 1)])?;
    assert!(is_tapped(&driver, "Raider"));
    until(&mut driver, Step::Main2)?;
    assert_eq!(driver.state().turn().step(), Step::DeclareAttackers);
    assert_eq!(
        driver.state().turn().combats(),
        2,
        "the added combat follows the first"
    );
    assert!(!is_tapped(&driver, "Raider"), "what attacked untaps");
    assert!(
        is_tapped(&driver, "Idler"),
        "what did not attack stays tapped"
    );
    driver.attack(&[("Raider", 1)])?;
    until(&mut driver, Step::Main2)?;
    assert_eq!(driver.state().turn().step(), Step::Main2);
    assert_eq!(driver.state().turn().combats(), 2);
    replays(&driver, &setup);
    Ok(())
}
