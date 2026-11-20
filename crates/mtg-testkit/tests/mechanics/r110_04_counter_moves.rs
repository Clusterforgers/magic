//! Goal: counter moves by kind and by "one or more" (CR 122.5), at tables of two to four seats: Goldberry takes one
//! counter of each kind it lacks from another permanent and none of a kind it has, and moves one or more of its
//! counters onto another permanent (never none while it has one), drawing a card only when one moved; the logs replay
//! to the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_cards::value::CounterKind;
use mtg_engine::choose::OptionChoice;
use mtg_engine::decision::{DecisionKind, PayChoice};
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_engine::{Answer, GameSetup, PresetPermanent, replay};
use mtg_testkit::cards::{free_creature, island};
use mtg_testkit::corpus::card;
use mtg_testkit::scenario::with_counters;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::pid;

const GOLDBERRY: &str = "Goldberry, River-Daughter";

fn replays(driver: &Driver, setup: &GameSetup) {
    let replayed = replay(setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
}

fn counters(driver: &Driver, name: &str, kind: &CounterKind) -> u32 {
    let id = driver.id_of(name).unwrap();
    driver
        .state()
        .object(id)
        .and_then(|object| object.permanent())
        .map_or(0, |held| held.counters_of(kind))
}

fn holding(permanent: PresetPermanent, held: &[(CounterKind, u32)]) -> PresetPermanent {
    held.iter().fold(permanent, |permanent, (kind, n)| {
        with_counters(permanent, kind.clone(), *n)
    })
}

/// Seat 0 controls Goldberry and Donor (a 2/2) with these counters, an Island, and a library to draw from.
fn table(seats: usize, goldberry: &[(CounterKind, u32)], donor: &[(CounterKind, u32)]) -> Scenario {
    Scenario::new(seats)
        .at(2, 0, Step::Main1)
        .battlefield(
            0,
            [
                holding(ready(card(GOLDBERRY).unwrap()), goldberry),
                holding(ready(free_creature("Donor", 2, 2)), donor),
                ready(island()),
            ],
        )
        .library(0, (0..5).map(|i| free_creature(&format!("Pad {i}"), 1, 1)))
}

/// Activates the ability of Goldberry at Donor and answers its questions: each counter pick takes `picks` in order.
fn activate(
    driver: &mut Driver,
    ability: usize,
    picks: &[usize],
) -> Result<Vec<usize>, ScriptError> {
    driver.begin_ability(GOLDBERRY, ability)?;
    let donor = driver.target_named("Donor")?;
    let mut offered = Vec::new();
    let mut picks = picks.iter();
    for _ in 0..100 {
        if driver.state().stack().is_empty()
            && driver.state().resolving().is_none()
            && matches!(driver.decision()?.kind, DecisionKind::Priority(_))
            && !offered.is_empty()
        {
            break;
        }
        match driver.decision()?.kind.clone() {
            DecisionKind::ChooseTargets(_) => {
                driver.choose_targets(&[donor])?;
            }
            DecisionKind::PayMana(_) => {
                driver.answer(Answer::PayMana(PayChoice::Auto))?;
            }
            DecisionKind::ChooseOption(options) => {
                offered.push(options.options.len());
                let pick = *picks.next().expect("a pick for each question");
                driver.answer(Answer::ChooseOption(OptionChoice(pick)))?;
            }
            DecisionKind::Priority(_) if driver.state().stack().is_empty() => {
                offered.push(0);
            }
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    Ok(offered)
}

#[rstest]
fn r110_goldberry_takes_one_of_each_kind_it_lacks(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let plus = CounterKind::PlusOnePlusOne;
    let charge = CounterKind::Charge;
    let oil = CounterKind::Other("oil".into());
    let donor = [(plus.clone(), 2), (charge.clone(), 1), (oil.clone(), 3)];
    let goldberry = [(plus.clone(), 1)];
    let setup = table(seats, &goldberry, &donor).setup().unwrap();
    let mut driver = Driver::from_scenario(table(seats, &goldberry, &donor))?;
    activate(&mut driver, 0, &[])?;
    assert_eq!(
        counters(&driver, GOLDBERRY, &plus),
        1,
        "a kind it has stays put"
    );
    assert_eq!(counters(&driver, GOLDBERRY, &charge), 1);
    assert_eq!(counters(&driver, GOLDBERRY, &oil), 1);
    assert_eq!(counters(&driver, "Donor", &plus), 2);
    assert_eq!(counters(&driver, "Donor", &charge), 0);
    assert_eq!(counters(&driver, "Donor", &oil), 2);
    replays(&driver, &setup);
    Ok(())
}

#[rstest]
fn r110_goldberry_moves_one_or_more_and_draws(
    #[values(2, 3, 4)] seats: usize,
    #[values(1, 2, 3)] moved: u32,
) -> Result<(), ScriptError> {
    let plus = CounterKind::PlusOnePlusOne;
    let charge = CounterKind::Charge;
    let goldberry = [(plus.clone(), 2), (charge.clone(), 1)];
    let setup = table(seats, &goldberry, &[]).setup().unwrap();
    let mut driver = Driver::from_scenario(table(seats, &goldberry, &[]))?;
    let hand = driver.state().zone(Zone::Hand(pid(0))).len();
    // Kinds come in their fixed order (+1/+1, then charge); "Stop" is the last option once one is taken.
    let picks: Vec<usize> = match moved {
        1 => vec![0, 2],
        2 => vec![0, 0, 1],
        _ => vec![0, 0, 0],
    };
    let offered = activate(&mut driver, 1, &picks)?;
    assert_eq!(
        offered.first(),
        Some(&2),
        "no Stop before the first counter"
    );
    let moved_plus = moved.min(2);
    assert_eq!(counters(&driver, "Donor", &plus), moved_plus);
    assert_eq!(counters(&driver, "Donor", &charge), moved - moved_plus);
    assert_eq!(counters(&driver, GOLDBERRY, &plus), 2 - moved_plus);
    assert_eq!(driver.state().zone(Zone::Hand(pid(0))).len(), hand + 1);
    replays(&driver, &setup);
    Ok(())
}

#[rstest]
fn r110_goldberry_without_counters_draws_nothing(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let setup = table(seats, &[], &[]).setup().unwrap();
    let mut driver = Driver::from_scenario(table(seats, &[], &[]))?;
    let hand = driver.state().zone(Zone::Hand(pid(0))).len();
    activate(&mut driver, 1, &[])?;
    assert_eq!(driver.state().zone(Zone::Hand(pid(0))).len(), hand);
    replays(&driver, &setup);
    Ok(())
}
