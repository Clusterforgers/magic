//! Goal: when Primeval Spawn leaves the battlefield its controller exiles the top ten cards of their library and casts
//! any number of spells among them with total mana value 10 or less for free: a pick over the total is refused, a pick
//! within it is cast whole, at two to four seats; the log replays to the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_engine::choose::{CardsChoice, YesNoChoice};
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::replay;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{bear, island};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::pid;

/// The names of the permanents seat 0 controls.
fn r6105_mine(driver: &Driver, name: &str) -> usize {
    driver
        .state()
        .zone(Zone::Battlefield)
        .iter()
        .filter_map(|id| driver.state().object(*id))
        .filter(|object| object.controller() == pid(0) && object.card().name == name)
        .count()
}

#[rstest]
fn r6105_spawn_casts_spells_with_total_mana_value_ten(
    #[values(2, 3, 4)] seats: usize,
    #[values(0, 1, 2)] bears: usize,
) -> Result<(), ScriptError> {
    let mut library = vec![card("Serra Angel").unwrap()];
    library.extend((0..4).map(|_| bear()));
    library.extend((0..7).map(|_| island()));
    let scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .battlefield(0, [ready(card("Primeval Spawn").unwrap()), ready(island()), ready(island())])
        .hand(0, [card("Into the Roil").unwrap()])
        .library(0, library);
    let setup = scenario.clone().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario)?;
    let spawn = driver.target_named("Primeval Spawn")?;
    driver.begin_cast("Into the Roil")?;
    let mut picked = false;
    for _ in 0..120 {
        if picked && driver.state().stack().is_empty() {
            break;
        }
        let decision = driver.decision()?.clone();
        match &decision.kind {
            DecisionKind::ChooseCards(options) if !picked => {
                assert_eq!(options.total.as_ref().map(|total| total.most), Some(10));
                let named = |name: &str| {
                    options
                        .from
                        .iter()
                        .copied()
                        .filter(|id| driver.state().object(*id).unwrap().card().name == name)
                        .collect::<Vec<_>>()
                };
                let giant = named("Serra Angel");
                let all_bears = named("Grizzly Bears");
                assert_eq!((giant.len(), all_bears.len()), (1, 4), "the top ten are exiled");
                // The angel and three bears are 11: refused.
                let over: Vec<_> = giant.iter().chain(&all_bears[..3]).copied().collect();
                assert!(driver.answer(Answer::ChooseCards(CardsChoice(over))).is_err());
                let pick: Vec<_> = giant.iter().chain(&all_bears[..bears]).copied().collect();
                driver.answer(Answer::ChooseCards(CardsChoice(pick)))?;
                picked = true;
            }
            DecisionKind::ChooseTargets(_) => {
                driver.choose_targets(&[spawn])?;
            }
            DecisionKind::PayMana(_) => {
                driver.answer(Answer::PayMana(PayChoice::Auto))?;
            }
            DecisionKind::YesNo(_) => {
                driver.answer(Answer::YesNo(YesNoChoice(true)))?;
            }
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    assert!(picked, "the pick is asked");
    assert_eq!(r6105_mine(&driver, "Serra Angel"), 1);
    assert_eq!(r6105_mine(&driver, "Grizzly Bears"), bears);
    let exiled = driver.state().zone(Zone::Exile).len();
    assert_eq!(exiled, 10 - 1 - bears, "the rest stay in exile");
    assert!(driver.state().zone(Zone::Hand(pid(0))).len() == 1, "the Spawn went back to its owner's hand");
    let replayed = replay(&setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
    Ok(())
}

/// Passes, answering every question passively, until the stack is empty.
fn r6105_settle(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..60 {
        if matches!(driver.decision()?.kind, DecisionKind::Priority(_)) && driver.state().stack().is_empty() {
            return Ok(());
        }
        driver.answer_passively()?;
    }
    Err(ScriptError::Stuck("settling".to_owned()))
}

#[rstest]
#[case::cast_with_mana(true)]
#[case::flickered(false)]
fn r6105_spawn_stays_only_when_cast_with_mana(
    #[values(2, 3, 4)] seats: usize,
    #[case] cast: bool,
) -> Result<(), ScriptError> {
    use mtg_testkit::cards::{forest, mountain, plains, swamp};
    let lands = [plains(), island(), swamp(), mountain(), forest(), plains(), plains(), plains(), plains(), plains()];
    let mut scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .library(0, vec![island(); 12])
        .battlefield(0, lands.into_iter().map(ready));
    scenario = if cast {
        scenario.hand(0, [card("Primeval Spawn").unwrap()])
    } else {
        scenario
            .hand(0, [card("Cloudshift").unwrap()])
            .battlefield(0, [ready(card("Primeval Spawn").unwrap())])
    };
    let mut driver = Driver::from_scenario(scenario)?;
    if cast {
        driver.cast("Primeval Spawn")?;
    } else {
        let spawn = driver.target_named("Primeval Spawn")?;
        driver.cast_at("Cloudshift", &[spawn])?;
    }
    r6105_settle(&mut driver)?;
    let on_battlefield = r6105_mine(&driver, "Primeval Spawn") == 1;
    assert_eq!(on_battlefield, cast, "a Spawn not cast with mana is exiled");
    let exiled = driver
        .state()
        .zone(Zone::Exile)
        .iter()
        .filter_map(|id| driver.state().object(*id))
        .any(|object| object.card().name == "Primeval Spawn");
    assert_eq!(exiled, !cast);
    Ok(())
}
