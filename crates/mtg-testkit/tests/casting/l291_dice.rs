//! Goal: "Whenever you roll a 1 or 2" triggers once for each die of its controller that shows one of its numbers and reads that number as "that many", never for another number or another player's die, at tables of two to four seats over several seeds; Dee Kay's 2 drains each opponent; and the log replays to the same state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::value::CounterKind;
use mtg_core::card::CardType;
use mtg_engine::decision::{Answer, PayChoice};
use mtg_engine::turn::Step;
use mtg_engine::{Event, replay};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::pid;

/// An artifact with "{0}: Roll a six-sided die."
fn l291_roller() -> Arc<CardDefinition> {
    let roll: Ability = serde_json::from_str(
        r#"{"Activated":{"cost":"{0}","effects":[{"RollDice":{"sides":6,"results":[]}}]}}"#,
    )
    .unwrap();
    Arc::new(
        CardDefinition::new("Die Roller")
            .with_type(CardType::Artifact)
            .with_rules(roll),
    )
}

/// Answers passively until the stack and the waiting triggers are empty; the dice rolled meanwhile, by seat.
fn l291_settle(driver: &mut Driver, rolled: &mut Vec<(usize, u32)>) -> Result<(), ScriptError> {
    for _ in 0..200 {
        let state = driver.state();
        if state.stack().is_empty()
            && state.resolving().is_none()
            && state.pending_triggers().is_empty()
        {
            return Ok(());
        }
        let events = driver.answer_passively()?;
        rolled.extend(events.iter().filter_map(|event| match event {
            Event::DieRolled { player, result, .. } => Some((player.index(), *result)),
            _ => None,
        }));
    }
    Err(ScriptError::Stuck("the stack".into()))
}

/// Seat `roller` controls a Die Roller; seat 0 controls `watcher`; it is `roller`'s first main phase.
fn l291_table(seats: usize, seed: u64, roller: usize, watcher: &str) -> Scenario {
    let scenario = Scenario::new(seats)
        .seed(seed)
        .at(2, roller, Step::Main1)
        .battlefield(roller, [ready(l291_roller())]);
    let mine = if roller == 0 {
        vec![ready(l291_roller()), ready(card(watcher).unwrap())]
    } else {
        vec![ready(card(watcher).unwrap())]
    };
    scenario.battlefield(0, mine)
}

/// Activates the Die Roller of the active seat `times` times; the dice rolled, by seat.
fn l291_roll(driver: &mut Driver, times: usize) -> Result<Vec<(usize, u32)>, ScriptError> {
    let mut rolled = Vec::new();
    for _ in 0..times {
        let events = driver.begin_activation("Die Roller")?;
        rolled.extend(events.iter().filter_map(|event| match event {
            Event::DieRolled { player, result, .. } => Some((player.index(), *result)),
            _ => None,
        }));
        driver.answer(Answer::PayMana(PayChoice::Auto))?;
        l291_settle(driver, &mut rolled)?;
    }
    Ok(rolled)
}

fn l291_replays(driver: &Driver, scenario: Scenario) {
    let setup = scenario.setup().unwrap();
    let replayed = replay(&setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.engine().state_hash());
}

#[rstest]
fn l291_a_one_or_two_puts_that_many_counters(
    #[values(2, 3, 4)] seats: usize,
    #[values(1, 2, 3)] seed: u64,
    #[values(0, 1)] roller: usize,
) -> Result<(), ScriptError> {
    let scenario = || l291_table(seats, seed, roller, "Atomwheel Acrobats");
    let mut driver = Driver::from_scenario(scenario())?;
    let rolled = l291_roll(&mut driver, 12)?;
    assert_eq!(rolled.len(), 12, "one die each time");
    let expected: u32 = rolled
        .iter()
        .filter(|(seat, result)| *seat == 0 && (1..=2).contains(result))
        .map(|(_, result)| result)
        .sum();
    let acrobats = driver.id_of("Atomwheel Acrobats")?;
    let counters = driver
        .state()
        .object(acrobats)
        .and_then(|object| object.permanent())
        .map_or(0, |permanent| {
            permanent.counters_of(&CounterKind::PlusOnePlusOne)
        });
    assert_eq!(counters, expected, "{rolled:?}");
    l291_replays(&driver, scenario());
    Ok(())
}

#[rstest]
fn l291_dee_kay_drains_on_a_two(
    #[values(2, 3, 4)] seats: usize,
    #[values(1, 2, 3)] seed: u64,
) -> Result<(), ScriptError> {
    let scenario = || l291_table(seats, seed, 0, "Dee Kay, Finder of the Lost");
    let mut driver = Driver::from_scenario(scenario())?;
    let life = |driver: &Driver, seat: usize| driver.state().player(pid(seat)).unwrap().life();
    let before: Vec<i32> = (0..seats).map(|seat| life(&driver, seat)).collect();
    let rolled = l291_roll(&mut driver, 12)?;
    let twos = i32::try_from(rolled.iter().filter(|(_, result)| *result == 2).count()).unwrap();
    assert_eq!(life(&driver, 0), before[0] + twos, "you gain 1 life once");
    for (seat, was) in before.iter().enumerate().skip(1) {
        assert_eq!(life(&driver, seat), was - twos, "seat {seat}");
    }
    l291_replays(&driver, scenario());
    Ok(())
}
