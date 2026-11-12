//! Goal: a counter of any kind removed as a cost (Fain, the Broker) is picked with `ChooseCards` among the payer's creatures that hold a counter, its kind is asked with `ChooseOption` only when the creature holds several and exactly that counter leaves; an effect that removes up to three counters of any kind asks for each one, offers "Stop" and never takes more than is there; at tables of two to four seats, and the log replays to the same state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use std::sync::Arc;

use common::name_of;
use mtg_cards::CardDefinition;
use mtg_cards::value::CounterKind;
use mtg_engine::choose::{CardsChoice, ChoiceOption, OptionChoice};
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::zones::Zone;
use mtg_engine::{GameSetup, replay};
use mtg_testkit::cards::free_creature;
use mtg_testkit::corpus::card;
use mtg_testkit::trigger_cards::creature_with;
use mtg_testkit::{Driver, Scenario, ScriptError, ready, with_counters};
use rstest::rstest;

fn w2f_shield() -> CounterKind {
    CounterKind::Other("shield".into())
}

/// A 1/1 with "{T}: Remove up to three counters from target permanent."
fn w2f_scrubber() -> Arc<CardDefinition> {
    let ability = serde_json::from_str(
        r#"{"Activated":{"cost":["Tap"],"targets":[{"what":{"Object":[]}}],"effects":[{"RemoveCounters":{"from":{"Target":0},"count":3,"up_to":true}}]}}"#,
    )
    .unwrap();
    creature_with("Scrubber", vec![ability])
}

/// The counters of each kind on the named permanent.
fn w2f_counters_on(driver: &Driver, name: &str, kind: &CounterKind) -> u32 {
    let state = driver.state();
    state
        .zone(Zone::Battlefield)
        .iter()
        .find(|id| name_of(state, **id) == name)
        .and_then(|id| state.object(*id))
        .and_then(|object| object.permanent())
        .map_or(0, |permanent| permanent.counters_of(kind))
}

fn w2f_replays(driver: &Driver, setup: &GameSetup) {
    let replayed = replay(setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
}

/// Passes until the stack is empty, answering each choice of option with the next of `answers`; the labels offered.
fn w2f_settle(driver: &mut Driver, answers: &[usize]) -> Result<Vec<Vec<String>>, ScriptError> {
    let mut answers = answers.iter();
    let mut offered = Vec::new();
    for _ in 0..100 {
        if let Ok(decision) = driver.decision()
            && let DecisionKind::ChooseOption(options) = &decision.kind
        {
            offered.push(w2f_labels(&options.options));
            let index = *answers.next().expect("an answer for each question");
            driver.answer(Answer::ChooseOption(OptionChoice(index)))?;
            continue;
        }
        if driver.state().stack().is_empty() {
            return Ok(offered);
        }
        driver.pass()?;
    }
    Err(ScriptError::Stuck("the stack".into()))
}

fn w2f_labels(options: &[ChoiceOption]) -> Vec<String> {
    options
        .iter()
        .map(|option| match option {
            ChoiceOption::Label(label) => label.clone(),
            other => panic!("{other:?}"),
        })
        .collect()
}

/// Fain beside a Wolf with two +1/+1 counters and a shield counter and an Ogre with one stun counter.
fn w2f_fain_table(seats: usize) -> Scenario {
    let wolf = with_counters(
        with_counters(
            ready(free_creature("Wolf", 2, 2)),
            CounterKind::PlusOnePlusOne,
            2,
        ),
        w2f_shield(),
        1,
    );
    let ogre = with_counters(
        ready(free_creature("Ogre", 3, 3)),
        CounterKind::Other("stun".into()),
        1,
    );
    Scenario::new(seats).battlefield(
        0,
        [
            ready(card("Fain, the Broker").unwrap()),
            wolf,
            ogre,
            ready(free_creature("Elk", 1, 1)),
        ],
    )
}

#[rstest]
#[case::the_wolf_loses_a_plus_one(Some(0), "Wolf", [1, 1, 1])]
#[case::the_wolf_loses_its_shield(Some(1), "Wolf", [2, 0, 1])]
#[case::the_ogre_is_not_asked(None, "Ogre", [2, 1, 0])]
fn w2f_fain_removes_the_counter_of_the_kind_picked(
    #[values(2, 3, 4)] seats: usize,
    #[case] kind: Option<usize>,
    #[case] picked: &str,
    #[case] left: [u32; 3],
) -> Result<(), ScriptError> {
    let scenario = w2f_fain_table(seats);
    let setup = scenario.clone().setup()?;
    let mut driver = Driver::from_scenario(scenario)?;
    driver.begin_ability("Fain, the Broker", 1)?;
    let DecisionKind::ChooseCards(options) = &driver.decision()?.kind else {
        panic!("a pick was expected");
    };
    let mut from: Vec<String> = options
        .from
        .iter()
        .map(|id| name_of(driver.state(), *id))
        .collect();
    from.sort();
    assert_eq!(from, ["Ogre", "Wolf"]);
    assert_eq!(options.text, "Remove a counter from a creature you control");
    let id = *options
        .from
        .iter()
        .find(|id| name_of(driver.state(), **id) == picked)
        .unwrap();
    driver.answer(Answer::ChooseCards(CardsChoice(vec![id])))?;
    if let Some(index) = kind {
        let DecisionKind::ChooseOption(options) = &driver.decision()?.kind else {
            panic!("a choice of kind was expected");
        };
        assert_eq!(
            w2f_labels(&options.options),
            ["+1/+1 counter", "shield counter"]
        );
        driver.answer(Answer::ChooseOption(OptionChoice(index)))?;
    }
    let counts = [
        w2f_counters_on(&driver, "Wolf", &CounterKind::PlusOnePlusOne),
        w2f_counters_on(&driver, "Wolf", &w2f_shield()),
        w2f_counters_on(&driver, "Ogre", &CounterKind::Other("stun".into())),
    ];
    assert_eq!(counts, left);
    w2f_settle(&mut driver, &[])?;
    let treasures = driver
        .state()
        .zone(Zone::Battlefield)
        .iter()
        .filter(|id| name_of(driver.state(), **id) == "Treasure")
        .count();
    assert_eq!(treasures, 1);
    w2f_replays(&driver, &setup);
    Ok(())
}

#[test]
fn w2f_no_creature_with_a_counter_offers_nothing() -> Result<(), ScriptError> {
    let driver = Driver::from_scenario(Scenario::new(2).battlefield(
        0,
        [
            ready(card("Fain, the Broker").unwrap()),
            ready(free_creature("Elk", 1, 1)),
        ],
    ))?;
    let DecisionKind::Priority(options) = &driver.decision()?.kind else {
        panic!("priority was expected");
    };
    assert!(
        options.abilities.iter().all(|ability| ability.ability != 1),
        "{options:?}"
    );
    Ok(())
}

#[rstest]
#[case::all_three(&[0, 0, 0], 3, [0, 0])]
#[case::stop_after_one(&[1, 1], 2, [2, 0])]
#[case::stop_at_once(&[2], 1, [2, 1])]
fn w2f_up_to_three_counters_of_any_kind(
    #[values(2, 3, 4)] seats: usize,
    #[case] answers: &[usize],
    #[case] questions: usize,
    #[case] left: [u32; 2],
) -> Result<(), ScriptError> {
    let wolf = with_counters(
        with_counters(
            ready(free_creature("Wolf", 2, 2)),
            CounterKind::PlusOnePlusOne,
            2,
        ),
        w2f_shield(),
        1,
    );
    let scenario = Scenario::new(seats)
        .battlefield(0, [ready(w2f_scrubber())])
        .battlefield(1, [wolf]);
    let setup = scenario.clone().setup()?;
    let mut driver = Driver::from_scenario(scenario)?;
    driver.begin_activation("Scrubber")?;
    let wolf = driver.target_named("Wolf")?;
    driver.choose_targets(&[wolf])?;
    let offered = w2f_settle(&mut driver, answers)?;
    assert_eq!(offered.len(), questions);
    assert_eq!(
        offered[0],
        ["+1/+1 counter", "shield counter", "Stop"],
        "the kinds held, then Stop"
    );
    let counts = [
        w2f_counters_on(&driver, "Wolf", &CounterKind::PlusOnePlusOne),
        w2f_counters_on(&driver, "Wolf", &w2f_shield()),
    ];
    assert_eq!(counts, left);
    w2f_replays(&driver, &setup);
    Ok(())
}
