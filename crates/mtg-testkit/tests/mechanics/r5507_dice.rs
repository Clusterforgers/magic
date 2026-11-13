//! Goal: a roll of a number of dice worked out as it happens ("roll three six-sided dice") plays through the whole stack at
//! 2 to 4 seats: each die is logged, the steps after it read how many came up odd and even (tokens and life), a
//! permanent entering with the total of its dice has that many counters, and the game replays to the same state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use crate::common;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::value::CounterKind;
use mtg_core::card::CardType;
use mtg_engine::decision::DecisionKind;
use mtg_engine::zones::Zone;
use mtg_engine::{Event, replay};
use mtg_testkit::cards::cost;
use mtg_testkit::{Driver, Scenario, ScriptError};
use rstest::rstest;

/// An instant for nothing that rolls three d6: a 1/1 token for each odd result, 1 life for each even result.
fn rolls() -> Arc<CardDefinition> {
    let rules: Ability = serde_json::from_str(
        r#"{"Spell":{"effects":[
            {"RollDice":{"sides":6,"results":[],"count":3}},
            {"CreateToken":{"token":{"Custom":{"name":"Robot","colors":["White"],"types":["Artifact","Creature"],"subtypes":["Robot"],"power":1,"toughness":1}},"count":{"DieResult":"Odd"}}},
            {"GainLife":{"amount":{"DieResult":"Even"}}}
        ]}}"#,
    )
    .unwrap();
    Arc::new(
        CardDefinition::new("Rolls")
            .with_type(CardType::Instant)
            .with_mana_cost(cost(0, &[]))
            .with_rules(rules),
    )
}

/// A 0/0 creature for nothing that rolls two d6 as it enters and enters with that many +1/+1 counters in all.
fn hydra() -> Arc<CardDefinition> {
    let rules: Ability = serde_json::from_str(
        r#"{"Replacement":{"when":{"Enters":{"what":["This"]}},"by":{"AsEntersWith":{"effects":[{"RollDice":{"sides":6,"results":[],"count":2}}],"counter":"+1/+1","count":{"DieResult":"Total"}}}}}"#,
    )
    .unwrap();
    Arc::new(
        CardDefinition::new("Hydra")
            .with_type(CardType::Creature)
            .with_power_toughness(0, 0)
            .with_mana_cost(cost(0, &[]))
            .with_rules(rules),
    )
}

fn table(seats: usize, seed: u64) -> Scenario {
    (0..seats)
        .fold(Scenario::new(seats), |scenario, seat| {
            scenario.library(
                seat,
                (0..10).map(|_| mtg_testkit::cards::forest()).collect(),
            )
        })
        .seed(seed)
        .hand(0, vec![rolls(), hydra()])
}

/// Passes priority until the stack is empty.
fn settle(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..200 {
        let Some(decision) = driver.engine().decision().cloned() else {
            return Ok(());
        };
        match decision.kind {
            DecisionKind::Priority(_)
                if driver.state().stack().is_empty() && driver.state().resolving().is_none() =>
            {
                return Ok(());
            }
            DecisionKind::Priority(_) => {
                driver.pass()?;
            }
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    Err(ScriptError::Stuck("the stack".into()))
}

/// The dice logged since `from` events.
fn dice(driver: &Driver, from: usize) -> Vec<u32> {
    driver.engine().events()[from..]
        .iter()
        .filter_map(|event| match event {
            Event::DieRolled { result, .. } => Some(*result),
            _ => None,
        })
        .collect()
}

#[rstest]
fn the_steps_after_the_roll_read_its_odd_and_even_results(
    #[values(2, 3, 4)] seats: usize,
    #[values(1, 7, 42)] seed: u64,
) -> Result<(), ScriptError> {
    let setup = table(seats, seed).setup().unwrap();
    let mut driver = Driver::from_scenario(table(seats, seed))?;
    let from = driver.engine().events().len();
    driver.cast("Rolls")?;
    settle(&mut driver)?;
    let rolled = dice(&driver, from);
    assert_eq!(rolled.len(), 3);
    let odd = rolled.iter().filter(|result| *result % 2 == 1).count();
    let robots = driver
        .state()
        .zone(Zone::Battlefield)
        .iter()
        .filter(|id| driver.state().object(**id).unwrap().card().name == "Robot")
        .count();
    assert_eq!(robots, odd);
    let life = driver.state().player(common::pid(0)).unwrap().life();
    assert_eq!(life, 20 + i32::try_from(3 - odd).unwrap());
    let replayed = replay(&setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
    Ok(())
}

#[rstest]
fn a_permanent_enters_with_the_total_of_its_dice(
    #[values(2, 3, 4)] seats: usize,
    #[values(1, 7, 42)] seed: u64,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table(seats, seed))?;
    let from = driver.engine().events().len();
    driver.cast("Hydra")?;
    settle(&mut driver)?;
    let rolled = dice(&driver, from);
    assert_eq!(rolled.len(), 2);
    let hydra = driver
        .state()
        .zone(Zone::Battlefield)
        .iter()
        .find_map(|id| {
            let object = driver.state().object(*id).unwrap();
            (object.card().name == "Hydra").then_some(object)
        })
        .expect("the Hydra survives with its counters");
    let counters = hydra
        .permanent()
        .unwrap()
        .counters_of(&CounterKind::PlusOnePlusOne);
    assert_eq!(counters, rolled.iter().sum::<u32>());
    Ok(())
}
