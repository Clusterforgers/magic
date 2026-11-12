//! Goal: a delayed trigger with targets of its own ("Whenever you cast a spell this turn, put a +1/+1 counter on target creature you control.") chooses its targets each time it triggers, does nothing when no target is legal, ends with the turn, and plays the same at tables of two, three and four seats; the log replays to the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::value::CounterKind;
use mtg_core::card::CardType;
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::turn::Step;
use mtg_engine::{Engine, GameSetup, replay};
use mtg_testkit::cards::free_creature;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// "{0}: Until end of turn, whenever you cast a spell, put a +1/+1 counter on target creature you control."
fn horn() -> Arc<CardDefinition> {
    let ability: Ability = serde_json::from_str(
        r#"{"Activated":{"cost":"{0}","effects":[{"DelayedTrigger":{"when":{"SpellCast":{"by":"You"}},"once":false,"targets":[{"what":{"Object":[{"Type":"Creature"},{"ControlledBy":"You"}]}}],"effects":[{"PutCounters":{"on":{"Target":0},"counter":"+1/+1"}}]}}]}}"#,
    )
    .unwrap();
    Arc::new(
        CardDefinition::new("Horn")
            .with_type(CardType::Artifact)
            .with_rules(ability),
    )
}

/// Passes and answers the first legal target until the stack is empty.
fn settle(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..60 {
        match &driver.decision()?.kind {
            DecisionKind::Priority(_) if driver.state().stack().is_empty() => return Ok(()),
            DecisionKind::ChooseTargets(options) => {
                let first = options.legal[0];
                driver.choose_targets(&[first])?;
            }
            DecisionKind::PayMana(_) => {
                driver.answer(Answer::PayMana(PayChoice::Auto))?;
            }
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    Err(ScriptError::Stuck("the stack did not empty".to_owned()))
}

fn counters(driver: &Driver, name: &str) -> u32 {
    let id = driver.id_of(name).unwrap();
    driver
        .state()
        .object(id)
        .and_then(|object| object.permanent())
        .map_or(0, |held| held.counters_of(&CounterKind::PlusOnePlusOne))
}

fn assert_replays(driver: &Driver, setup: &GameSetup) {
    let log = driver.engine().events().to_vec();
    let replayed = replay(setup, &log).unwrap();
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
    let resumed = Engine::resume(setup.clone(), log).unwrap();
    assert_eq!(resumed.state_hash(), driver.state().state_hash());
}

#[rstest]
#[case::one_spell(1, 1)]
#[case::two_spells(2, 2)]
#[case::three_spells(3, 3)]
fn each_spell_cast_this_turn_puts_a_counter_on_the_chosen_target(
    #[values(2, 3, 4)] seats: usize,
    #[case] spells: usize,
    #[case] expected: u32,
) -> Result<(), ScriptError> {
    let table = || {
        let hand: Vec<_> = (0..spells)
            .map(|at| free_creature(&format!("Caster{at}"), 1, 1))
            .collect();
        Scenario::new(seats)
            .at(1, 0, Step::Main1)
            .battlefield(0, [ready(horn()), ready(free_creature("Bear", 2, 2))])
            .battlefield(1, [ready(free_creature("Rival", 2, 2))])
            .hand(0, hand)
    };
    let mut driver = Driver::from_scenario(table())?;
    driver.begin_ability("Horn", 0)?;
    settle(&mut driver)?;
    for at in 0..spells {
        driver.cast(&format!("Caster{at}"))?;
        settle(&mut driver)?;
    }
    // The first spell's trigger can only target the Bear; later ones may target a creature cast before.
    let total: u32 = std::iter::once("Bear".to_owned())
        .chain((0..spells).map(|at| format!("Caster{at}")))
        .map(|name| counters(&driver, &name))
        .sum();
    assert_eq!(total, expected);
    assert!(counters(&driver, "Bear") >= 1);
    assert_eq!(counters(&driver, "Rival"), 0);
    assert_replays(&driver, &table().setup().unwrap());
    Ok(())
}

#[rstest]
fn with_no_creature_to_target_the_trigger_does_nothing(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let table = || {
        Scenario::new(seats)
            .at(1, 0, Step::Main1)
            .battlefield(0, [ready(horn())])
            .battlefield(1, [ready(free_creature("Rival", 2, 2))])
            .hand(0, [free_creature("Caster", 1, 1)])
    };
    let mut driver = Driver::from_scenario(table())?;
    driver.begin_ability("Horn", 0)?;
    settle(&mut driver)?;
    driver.cast("Caster")?;
    settle(&mut driver)?;
    assert_eq!(counters(&driver, "Caster"), 0);
    assert_eq!(counters(&driver, "Rival"), 0);
    assert_replays(&driver, &table().setup().unwrap());
    Ok(())
}

#[rstest]
fn the_trigger_ends_with_the_turn(#[values(2, 3, 4)] seats: usize) -> Result<(), ScriptError> {
    let table = || {
        Scenario::new(seats)
            .at(1, 0, Step::Main1)
            .battlefield(0, [ready(horn()), ready(free_creature("Bear", 2, 2))])
            .hand(0, [free_creature("Caster", 1, 1)])
    };
    let mut driver = Driver::from_scenario(table())?;
    driver.begin_ability("Horn", 0)?;
    settle(&mut driver)?;
    assert_eq!(driver.state().delayed().len(), 1);
    while driver.state().turn().number() == 1 {
        driver.answer_passively()?;
    }
    assert_eq!(driver.state().delayed().len(), 0);
    assert_eq!(counters(&driver, "Bear"), 0);
    Ok(())
}
