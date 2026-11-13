//! Goal: "Devour X, where X is the number of creatures devoured this way" (Thromok the Insatiable) puts X +1/+1 counters
//! on it for each creature devoured, X² in all (a 0/0 that devours nothing dies), at tables of two to four seats; the log replays to the same state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common::casting::lands;
use mtg_cards::value::CounterKind;
use mtg_engine::choose::CardsChoice;
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::replay;
use mtg_engine::turn::Step;
use mtg_testkit::cards::{forest, free_creature, mountain};
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

const THROMOK: &str = "Thromok the Insatiable";

fn table(seats: usize, pets: usize) -> Scenario {
    let mut permanents = lands(3, mountain);
    permanents.extend(lands(2, forest));
    permanents.extend((0..pets).map(|_| ready(free_creature("Pet", 1, 1))));
    let scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .hand_named(0, [THROMOK])
        .battlefield(0, permanents);
    (0..seats).fold(scenario, |scenario, seat| {
        scenario
            .battlefield(seat.max(1), [ready(free_creature("Foe", 2, 2))])
            .library(seat, (0..8).map(|_| forest()))
    })
}

/// Passes priority until a decision other than priority is pending or the stack is empty.
fn run_until_asked(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..12 {
        let priority = matches!(driver.decision()?.kind, DecisionKind::Priority(_));
        if !priority || driver.state().stack().is_empty() {
            break;
        }
        driver.pass()?;
    }
    Ok(())
}

#[rstest]
fn thromok_enters_with_x_counters_for_each_of_x_creatures_devoured(
    #[values(2, 3, 4)] seats: usize,
    #[values(0, 1, 2, 3)] devoured: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table(seats, 3))?;
    driver.begin_cast(THROMOK)?;
    driver.expect_decision(0, "pay-mana")?;
    driver.answer(Answer::PayMana(PayChoice::Auto))?;
    run_until_asked(&mut driver)?;
    let DecisionKind::ChooseCards(options) = &driver.decision()?.kind else {
        panic!("the creatures to devour are asked");
    };
    assert_eq!(options.from.len(), 3, "only the controller's own Pets");
    let chosen = options.from[..devoured].to_vec();
    driver.answer(Answer::ChooseCards(CardsChoice(chosen)))?;
    run_until_asked(&mut driver)?;
    if devoured == 0 {
        // A 0/0 that devoured nothing is put into its owner's graveyard (CR 704.5f).
        assert!(driver.id_of(THROMOK).is_err());
        return Ok(());
    }
    let thromok = driver.id_of(THROMOK)?;
    let counters = driver
        .state()
        .object(thromok)
        .and_then(|object| object.permanent())
        .map_or(0, |permanent| {
            permanent.counters_of(&CounterKind::PlusOnePlusOne)
        });
    let x = u32::try_from(devoured).unwrap();
    assert_eq!(counters, x * x);
    let replayed = replay(driver.engine().setup(), driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
    Ok(())
}
