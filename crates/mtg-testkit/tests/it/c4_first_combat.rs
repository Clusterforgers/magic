//! Goal: "if it's the first combat phase of the turn" holds in the first combat phase only, at tables of two, three and four seats: a creature whose attack trigger adds a combat phase under that condition gives the turn exactly one more combat, the creatures it untapped attack again without adding another, the added combat comes before the second main phase, and the log replays to the state it was played to.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_engine::turn::Step;
use mtg_engine::{Event, replay};
use mtg_testkit::cards::free_creature;
use mtg_testkit::trigger_cards::with_rules;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// The turn the scenarios start in.
const START: u32 = 4;

/// A 2/2 whose attack adds a combat phase the first time it attacks in a turn: "Whenever this creature attacks, if it's
/// the first combat phase of the turn, untap all attacking creatures. After this phase, there is an additional combat
/// phase."
fn raider(json_if: &str) -> Arc<CardDefinition> {
    let trigger: Ability = serde_json::from_str(&format!(
        r#"{{"Triggered":{{"when":{{"Attacks":{{"what":["This"]}}}}{json_if},"effects":[{{"Untap":{{"what":{{"Each":[{{"Type":"Creature"}},"Attacking"]}}}}}},{{"AddPhase":{{"phases":["Combat"]}}}}]}}}}"#
    ))
    .unwrap();
    with_rules(&ready_card(), vec![trigger])
}

fn ready_card() -> Arc<CardDefinition> {
    free_creature("Raider", 2, 2)
}

/// The combat phases that began from the turn's first main phase on, and the attacks declared.
fn combats_and_attacks(driver: &Driver) -> (usize, usize) {
    let events = driver.engine().events();
    let combats = events
        .iter()
        .filter(|event| {
            matches!(
                event,
                Event::StepBegan {
                    step: Step::BeginCombat
                }
            )
        })
        .count();
    let attacks = events
        .iter()
        .filter(|event| matches!(event, Event::AttackersDeclared { .. }))
        .count();
    (combats, attacks)
}

#[rstest]
fn a_combat_added_under_the_first_combat_condition_is_added_once(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = Scenario::new(seats)
        .at(START, 0, Step::Main1)
        .battlefield(0, [ready(raider(r#","if":"FirstCombat""#))]);
    let setup = scenario.clone().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario)?;
    driver.pass_until(Step::DeclareAttackers)?;
    driver.attack(&[("Raider", 1)])?;
    // The added combat begins, the untapped creature attacks again, and the condition no longer holds.
    for _ in 0..4_000 {
        if driver.state().turn().step() == Step::DeclareAttackers
            && driver.state().turn().combats() == 2
        {
            break;
        }
        driver.answer_passively()?;
    }
    assert_eq!(driver.state().turn().combats(), 2);
    driver.attack(&[("Raider", 1)])?;
    for _ in 0..4_000 {
        if driver.state().turn().step() == Step::Main2 {
            break;
        }
        driver.answer_passively()?;
    }
    assert_eq!(driver.state().turn().step(), Step::Main2);
    assert_eq!(driver.state().turn().combats(), 2);
    assert_eq!(combats_and_attacks(&driver), (2, 2));
    let replayed = replay(&setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.engine().state_hash());
    Ok(())
}
