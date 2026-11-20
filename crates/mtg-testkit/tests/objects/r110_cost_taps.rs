//! Goal: the permanents tapped this turn to pay for a source's abilities (Vodalian War Machine), at 2 to 4 seats: the
//! Merfolk tapped for either of its abilities are destroyed as it dies, an untapped Merfolk stays, and the log replays
//! to the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_cards::modify::Duration;
use mtg_engine::decision::DecisionKind;
use mtg_engine::effects::Modification;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::copying::replays;
use crate::common::{names, pid};

const MACHINE: &str = "Vodalian War Machine";
const SPY: &str = "Merfolk Spy";

/// Answers every question passively until the stack is empty.
fn settle_stack(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..200 {
        let decision = driver.decision()?.clone();
        if matches!(decision.kind, DecisionKind::Priority(_)) && driver.state().stack().is_empty() {
            return Ok(());
        }
        driver.answer_passively()?;
    }
    Err(ScriptError::Stuck("the stack".into()))
}

fn spies(driver: &Driver, zone: Zone) -> usize {
    names(driver.state(), zone)
        .iter()
        .filter(|name| *name == SPY)
        .count()
}

#[rstest]
fn vodalian_war_machine_destroys_the_merfolk_tapped_for_its_abilities(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = || {
        Scenario::new(seats).at(1, 0, Step::Main1).battlefield(
            0,
            [
                ready(card(MACHINE).unwrap()),
                ready(card(SPY).unwrap()),
                ready(card(SPY).unwrap()),
                ready(card(SPY).unwrap()),
            ],
        )
    };
    let setup = scenario().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario())?;
    // One Merfolk for each of its two abilities.
    for ability in [1, 2] {
        driver.begin_ability(MACHINE, ability)?;
        settle_stack(&mut driver)?;
    }
    let tapped = driver
        .state()
        .zone(Zone::Battlefield)
        .iter()
        .filter(|id| {
            driver.state().object(**id).is_some_and(|object| {
                object.card().name == SPY
                    && object
                        .permanent()
                        .is_some_and(mtg_engine::object::PermanentState::tapped)
            })
        })
        .count();
    assert_eq!(tapped, 2);
    let machine = driver.id_of(MACHINE)?;
    let on_machine = driver.on(machine);
    driver.add_effect(
        on_machine,
        Modification::SetToughness(-10),
        Duration::UntilEndOfTurn,
    )?;
    // The state-based actions are checked as a player would next receive priority.
    driver.pass()?;
    assert!(names(driver.state(), Zone::Graveyard(pid(0))).contains(&MACHINE.to_owned()));
    settle_stack(&mut driver)?;
    assert_eq!(spies(&driver, Zone::Graveyard(pid(0))), 2);
    assert_eq!(spies(&driver, Zone::Battlefield), 1);
    replays(&driver, &setup);
    Ok(())
}
