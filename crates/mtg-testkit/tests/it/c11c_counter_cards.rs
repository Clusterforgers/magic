//! Goal: a counter removed from a permanent the payer controls is picked with `ChooseCards` among their permanents of the kind that hold such a counter (the source included, never another player's), costs exactly one counter of that kind, and the ability is not offered when no permanent holds one; Angelic Sleuth investigates when a permanent that leaves had counters on it and never otherwise; at tables of two to four seats, and the log replays to the same state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use std::sync::Arc;

use common::name_of;
use mtg_cards::CardDefinition;
use mtg_cards::value::CounterKind;
use mtg_engine::choose::CardsChoice;
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::zones::Zone;
use mtg_engine::{GameSetup, replay};
use mtg_testkit::cards::free_creature;
use mtg_testkit::corpus::card;
use mtg_testkit::part_cards::sacrificer;
use mtg_testkit::trigger_cards::creature_with;
use mtg_testkit::{Driver, Scenario, ScriptError, ready, with_counters};
use rstest::rstest;

/// A 1/1 with "Remove a +1/+1 counter from a creature you control: Draw a card."
fn c11c_remover() -> Arc<CardDefinition> {
    let ability = serde_json::from_str(
        r#"{"Activated":{"cost":[{"RemoveCounters":{"counter":"+1/+1","from":[{"Type":"Creature"}]}}],"effects":[{"Draw":{}}]}}"#,
    )
    .unwrap();
    creature_with("Remover", vec![ability])
}

fn c11c_counters_on(driver: &Driver, name: &str) -> u32 {
    let state = driver.state();
    state
        .zone(Zone::Battlefield)
        .iter()
        .find(|id| name_of(state, **id) == name)
        .and_then(|id| state.object(*id))
        .and_then(|object| object.permanent())
        .map_or(0, |permanent| {
            permanent.counters_of(&CounterKind::PlusOnePlusOne)
        })
}

fn c11c_replays(driver: &Driver, setup: &GameSetup) {
    let replayed = replay(setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
}

fn c11c_settle(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..100 {
        if driver.state().stack().is_empty() {
            return Ok(());
        }
        driver.pass()?;
    }
    Err(ScriptError::Stuck("the stack".into()))
}

#[rstest]
fn c11c_a_counter_of_a_picked_creature_pays_the_cost(
    #[values(2, 3, 4)] seats: usize,
    #[values("Wolf", "Ogre")] picked: &str,
) -> Result<(), ScriptError> {
    let foe = with_counters(
        ready(free_creature("Giant", 5, 5)),
        CounterKind::PlusOnePlusOne,
        1,
    );
    let scenario = Scenario::new(seats)
        .battlefield(
            0,
            [
                ready(c11c_remover()),
                with_counters(
                    ready(free_creature("Wolf", 2, 2)),
                    CounterKind::PlusOnePlusOne,
                    2,
                ),
                with_counters(
                    ready(free_creature("Ogre", 3, 3)),
                    CounterKind::PlusOnePlusOne,
                    1,
                ),
                ready(free_creature("Elk", 1, 1)),
            ],
        )
        .battlefield(1, [foe]);
    let setup = scenario.clone().setup()?;
    let mut driver = Driver::from_scenario(scenario)?;
    let before = (
        c11c_counters_on(&driver, "Wolf"),
        c11c_counters_on(&driver, "Ogre"),
    );
    driver.begin_activation("Remover")?;
    let DecisionKind::ChooseCards(options) = &driver.decision().unwrap().kind else {
        panic!("a pick was expected");
    };
    let mut from: Vec<String> = options
        .from
        .iter()
        .map(|id| name_of(driver.state(), *id))
        .collect();
    from.sort();
    assert_eq!(from, ["Ogre", "Wolf"]);
    assert_eq!(
        options.text,
        "Remove a +1/+1 counter from a creature you control"
    );
    let id = *options
        .from
        .iter()
        .find(|id| name_of(driver.state(), **id) == picked)
        .unwrap();
    driver.answer(Answer::ChooseCards(CardsChoice(vec![id])))?;
    let after = (
        c11c_counters_on(&driver, "Wolf"),
        c11c_counters_on(&driver, "Ogre"),
    );
    let expected = if picked == "Wolf" {
        (before.0 - 1, before.1)
    } else {
        (before.0, before.1 - 1)
    };
    assert_eq!(after, expected);
    c11c_settle(&mut driver)?;
    c11c_replays(&driver, &setup);
    Ok(())
}

#[test]
fn c11c_no_permanent_with_the_counter_offers_nothing() -> Result<(), ScriptError> {
    let driver = Driver::from_scenario(Scenario::new(2).battlefield(
        0,
        [ready(c11c_remover()), ready(free_creature("Elk", 1, 1))],
    ))?;
    let DecisionKind::Priority(options) = &driver.decision().unwrap().kind else {
        panic!("priority was expected");
    };
    assert!(options.abilities.is_empty(), "{options:?}");
    Ok(())
}

#[rstest]
fn c11c_angelic_sleuth_investigates_for_a_permanent_that_had_counters(
    #[values(2, 3, 4)] seats: usize,
    #[values(("Wolf", true), ("Ogre", false))] case: (&str, bool),
) -> Result<(), ScriptError> {
    let (sacrificed, investigates) = case;
    let scenario = Scenario::new(seats).battlefield(
        0,
        [
            ready(card("Angelic Sleuth").unwrap()),
            ready(sacrificer("Seer")),
            with_counters(
                ready(free_creature("Wolf", 2, 2)),
                CounterKind::Other("charge".into()),
                1,
            ),
            ready(free_creature("Ogre", 3, 3)),
        ],
    );
    let setup = scenario.clone().setup()?;
    let mut driver = Driver::from_scenario(scenario)?;
    driver.begin_activation("Seer")?;
    let DecisionKind::ChooseCards(options) = &driver.decision().unwrap().kind else {
        panic!("a pick was expected");
    };
    let id = *options
        .from
        .iter()
        .find(|id| name_of(driver.state(), **id) == sacrificed)
        .unwrap();
    driver.answer(Answer::ChooseCards(CardsChoice(vec![id])))?;
    c11c_settle(&mut driver)?;
    let clues = driver
        .state()
        .zone(Zone::Battlefield)
        .iter()
        .filter(|id| name_of(driver.state(), **id) == "Clue")
        .count();
    assert_eq!(clues, usize::from(investigates), "{sacrificed}");
    c11c_replays(&driver, &setup);
    Ok(())
}
