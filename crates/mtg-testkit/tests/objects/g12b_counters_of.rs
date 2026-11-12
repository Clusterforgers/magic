//! Goal: "put its counters on target creature you control" puts on the target as many of each kind as the creature held as it left the battlefield, whether it died to a cost of another ability, to a cost of its own, or is another creature whose death triggered the ability; a creature that held none puts nothing; at tables of two to four seats, and the log replays to the same state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use std::sync::Arc;

use common::name_of;
use mtg_cards::CardDefinition;
use mtg_cards::value::CounterKind;
use mtg_engine::choose::CardsChoice;
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::zones::Zone;
use mtg_engine::{GameSetup, PresetPermanent, replay};
use mtg_testkit::cards::free_creature;
use mtg_testkit::part_cards::sacrificer;
use mtg_testkit::trigger_cards::creature_with;
use mtg_testkit::{Driver, Scenario, ScriptError, ready, with_counters};
use rstest::rstest;

const TO_BEAR: &str =
    r#""targets":[{"what":{"Object":[{"Type":"Creature"},{"ControlledBy":"You"}]}}]"#;

/// "When this creature dies, put its counters on target creature you control."
fn g12b_pupil() -> Arc<CardDefinition> {
    let ability = format!(
        r#"{{"Triggered":{{"when":{{"ZoneChange":{{"from":"Battlefield","to":"Graveyard","what":["This"]}}}},"effects":[{{"PutCountersOf":{{"on":{{"Target":0}},"of":"This"}}}}],{TO_BEAR}}}}}"#
    );
    creature_with("Pupil", vec![serde_json::from_str(&ability).unwrap()])
}

/// "Sacrifice this creature: Put its counters on target creature you control."
fn g12b_giver() -> Arc<CardDefinition> {
    let ability = format!(
        r#"{{"Activated":{{"cost":["SacrificeSelf"],"effects":[{{"PutCountersOf":{{"on":{{"Target":0}},"of":"This"}}}}],{TO_BEAR}}}}}"#
    );
    creature_with("Giver", vec![serde_json::from_str(&ability).unwrap()])
}

/// "Whenever another creature you control dies, put its counters on this creature."
fn g12b_colony() -> Arc<CardDefinition> {
    let ability = r#"{"Triggered":{"when":{"ZoneChange":{"from":"Battlefield","to":"Graveyard","what":[{"Type":"Creature"},{"ControlledBy":"You"},"Other"]}},"effects":[{"PutCountersOf":{"on":"This","of":"Triggering"}}]}}"#;
    creature_with("Colony", vec![serde_json::from_str(ability).unwrap()])
}

fn g12b_counters(driver: &Driver, name: &str) -> Vec<(CounterKind, u32)> {
    let state = driver.state();
    state
        .zone(Zone::Battlefield)
        .iter()
        .find(|id| name_of(state, **id) == name)
        .and_then(|id| state.object(*id))
        .and_then(|object| object.permanent())
        .map(|permanent| {
            permanent
                .counters()
                .map(|(kind, n)| (kind.clone(), n))
                .collect()
        })
        .unwrap_or_default()
}

/// Passes until the stack is empty, aiming each trigger at the Bear.
fn g12b_settle(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..100 {
        if matches!(
            driver.decision().map(|d| &d.kind),
            Ok(DecisionKind::ChooseTargets(_))
        ) {
            let bear = driver.target_named("Bear")?;
            driver.choose_targets(&[bear])?;
            continue;
        }
        if driver.state().stack().is_empty() {
            return Ok(());
        }
        driver.pass()?;
    }
    Err(ScriptError::Stuck("the stack".into()))
}

fn g12b_replays(driver: &Driver, setup: &GameSetup) {
    let replayed = replay(setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
}

fn g12b_held() -> Vec<(CounterKind, u32)> {
    vec![
        (CounterKind::PlusOnePlusOne, 2),
        (CounterKind::Other("charge".into()), 1),
    ]
}

fn g12b_loaded(card: Arc<CardDefinition>) -> PresetPermanent {
    let card = with_counters(ready(card), CounterKind::PlusOnePlusOne, 2);
    with_counters(card, CounterKind::Other("charge".into()), 1)
}

/// Sacrifices `name` to the Seer.
fn g12b_sacrifice(driver: &mut Driver, name: &str) -> Result<(), ScriptError> {
    driver.begin_activation("Seer")?;
    let DecisionKind::ChooseCards(options) = &driver.decision().unwrap().kind else {
        panic!("a pick was expected");
    };
    let id = *options
        .from
        .iter()
        .find(|id| name_of(driver.state(), **id) == name)
        .unwrap();
    driver.answer(Answer::ChooseCards(CardsChoice(vec![id])))?;
    Ok(())
}

#[rstest]
fn g12b_a_dying_creature_gives_its_counters(
    #[values(2, 3, 4)] seats: usize,
    #[values(true, false)] loaded: bool,
) -> Result<(), ScriptError> {
    let pupil = if loaded {
        g12b_loaded(g12b_pupil())
    } else {
        ready(g12b_pupil())
    };
    let scenario = Scenario::new(seats).battlefield(
        0,
        [
            ready(sacrificer("Seer")),
            pupil,
            ready(free_creature("Bear", 2, 2)),
        ],
    );
    let setup = scenario.clone().setup()?;
    let mut driver = Driver::from_scenario(scenario)?;
    g12b_sacrifice(&mut driver, "Pupil")?;
    g12b_settle(&mut driver)?;
    let expected = if loaded { g12b_held() } else { Vec::new() };
    assert_eq!(g12b_counters(&driver, "Bear"), expected);
    g12b_replays(&driver, &setup);
    Ok(())
}

#[rstest]
fn g12b_a_creature_sacrificed_to_its_own_ability_gives_its_counters(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = Scenario::new(seats).battlefield(
        0,
        [
            g12b_loaded(g12b_giver()),
            ready(free_creature("Bear", 2, 2)),
        ],
    );
    let setup = scenario.clone().setup()?;
    let mut driver = Driver::from_scenario(scenario)?;
    driver.begin_activation("Giver")?;
    g12b_settle(&mut driver)?;
    assert_eq!(g12b_counters(&driver, "Bear"), g12b_held());
    g12b_replays(&driver, &setup);
    Ok(())
}

#[rstest]
fn g12b_another_creature_that_dies_gives_its_counters(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = Scenario::new(seats).battlefield(
        0,
        [
            ready(sacrificer("Seer")),
            ready(g12b_colony()),
            g12b_loaded(free_creature("Wolf", 2, 2)),
        ],
    );
    let setup = scenario.clone().setup()?;
    let mut driver = Driver::from_scenario(scenario)?;
    g12b_sacrifice(&mut driver, "Wolf")?;
    g12b_settle(&mut driver)?;
    assert_eq!(g12b_counters(&driver, "Colony"), g12b_held());
    g12b_replays(&driver, &setup);
    Ok(())
}
