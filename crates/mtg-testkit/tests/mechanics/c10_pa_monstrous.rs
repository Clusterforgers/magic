//! Goal: X in a trigger on becoming monstrous is the X of the monstrosity (CR 701.37c) at tables of two, three and four seats: Hydra Broodmaster makes X X/X Hydras for the X it was activated with, also when a replacement effect changes the counters it gets, and the log replays to the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_core::color::Color;
use mtg_engine::choose::NumberChoice;
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_engine::{PresetPermanent, replay};
use mtg_testkit::cards::land;
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::pid;

const BROODMASTER: &str = "Hydra Broodmaster";

/// The Hydra tokens seat 0 controls, as (power, toughness) each.
fn c10_pa_hydras(driver: &Driver) -> Vec<(i32, i32)> {
    let state = driver.state();
    state
        .zone(Zone::Battlefield)
        .iter()
        .filter_map(|id| state.object(*id))
        .filter(|object| object.controller() == pid(0) && object.card().name == "Hydra")
        .map(|object| {
            let chars = mtg_engine::characteristics::characteristics(state, object.id()).unwrap();
            (chars.power.unwrap(), chars.toughness.unwrap())
        })
        .collect()
}

#[rstest]
#[case::one(1, None)]
#[case::two(2, None)]
#[case::three(3, None)]
#[case::scales_add_a_counter_not_a_hydra(2, Some("Hardened Scales"))]
fn c10_pa_the_broodmaster_makes_x_hydras_of_size_x(
    #[case] x: u32,
    #[case] extra: Option<&str>,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = || {
        let mut permanents: Vec<PresetPermanent> = vec![ready(card(BROODMASTER).unwrap())];
        permanents.extend((0..7).map(|_| ready(land(Color::Green))));
        if let Some(name) = extra {
            permanents.push(ready(card(name).unwrap()));
        }
        Scenario::new(seats)
            .at(4, 0, Step::Main1)
            .battlefield(0, permanents)
    };
    let setup = scenario().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario())?;
    driver.begin_activation(BROODMASTER)?;
    loop {
        match &driver.decision()?.kind {
            DecisionKind::PayMana(_) => break,
            DecisionKind::ChooseNumber(_) => {
                driver.answer(Answer::ChooseNumber(NumberChoice::Number(x)))?;
            }
            other => panic!("unexpected {}", other.name()),
        }
    }
    driver.answer(Answer::PayMana(PayChoice::Auto))?;
    for _ in 0..40 {
        if driver.state().stack().is_empty() && driver.state().resolving().is_none() {
            break;
        }
        driver.answer_passively()?;
    }
    let size = i32::try_from(x).unwrap();
    assert_eq!(c10_pa_hydras(&driver), vec![(size, size); x as usize]);
    let replayed = replay(&setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
    Ok(())
}
