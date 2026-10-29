//! Goal: the popular cards the sweep reads whole play as printed at tables of two, three and four seats: Syr Konrad deals 1 damage to each opponent when another creature dies, The Great Henge puts a +1/+1 counter on a nontoken creature that enters and draws a card, and Worldly Tutor puts the creature it finds on top of the library.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_cards::value::CounterKind;
use mtg_core::ids::PlayerId;
use mtg_engine::choose::CardsChoice;
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{burn, forest, mountain, swamp};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::copying::replays;
use crate::common::{name_of, names, on_battlefield};

/// Passes priority until the stack is empty and nothing is resolving.
fn t15a_settle(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..200 {
        if driver.state().stack().is_empty() && driver.state().resolving().is_none() {
            return Ok(());
        }
        driver.answer_passively()?;
    }
    Err(ScriptError::Stuck("the stack".into()))
}

/// Settles the stack, taking the first card named `creature` whenever cards are offered.
fn t15a_settle_taking(driver: &mut Driver, creature: &str) -> Result<(), ScriptError> {
    for _ in 0..200 {
        if driver.state().stack().is_empty() && driver.state().resolving().is_none() {
            return Ok(());
        }
        if let DecisionKind::ChooseCards(options) = &driver.decision()?.kind {
            let chosen = options
                .from
                .iter()
                .copied()
                .filter(|id| name_of(driver.state(), *id) == creature)
                .take(1)
                .collect();
            driver.answer(Answer::ChooseCards(CardsChoice(chosen)))?;
        } else {
            driver.answer_passively()?;
        }
    }
    Err(ScriptError::Stuck("the stack".into()))
}

/// A table of `seats` where every seat but the first holds a Swamp.
fn t15a_table(seats: usize) -> Scenario {
    let mut scenario = Scenario::new(seats).at(1, 0, Step::Main1);
    for seat in 1..seats {
        scenario = scenario.battlefield(seat, [ready(swamp())]);
    }
    scenario
}

#[rstest]
fn t15a_syr_konrad_hurts_each_opponent_when_another_creature_dies(
    #[values(2, 3, 4)] seats: usize,
    #[values(1, 2)] dying: usize,
) -> Result<(), ScriptError> {
    let zombie = || ready(card("Dregscape Zombie").unwrap());
    let scenario = t15a_table(seats)
        .hand(0, [burn("Bolt", 3), burn("Bolt", 3)])
        .battlefield(
            0,
            [
                ready(card("Syr Konrad, the Grim").unwrap()),
                ready(mountain()),
            ]
            .into_iter()
            .chain((0..dying).map(|_| zombie()))
            .chain([ready(mountain())]),
        );
    let setup = scenario.clone().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario)?;
    for _ in 0..dying {
        let target = driver.target_named("Dregscape Zombie")?;
        driver.cast_at("Bolt", &[target])?;
        t15a_settle(&mut driver)?;
    }
    let hurt = i32::try_from(dying).unwrap();
    driver.expect_life(0, 20)?;
    for seat in 1..seats {
        driver.expect_life(seat, 20 - hurt)?;
    }
    replays(&driver, &setup);
    Ok(())
}

#[rstest]
fn t15a_the_great_henge_counts_a_nontoken_creature_and_draws(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = t15a_table(seats)
        .hand(0, [card("Dregscape Zombie").unwrap()])
        .library(0, (0..4).map(|_| swamp()))
        .battlefield(
            0,
            [ready(card("The Great Henge").unwrap())]
                .into_iter()
                .chain((0..3).map(|_| ready(swamp()))),
        );
    let setup = scenario.clone().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario)?;
    driver.cast_tapping("Dregscape Zombie", &["Swamp", "Swamp"])?;
    t15a_settle(&mut driver)?;
    let id = on_battlefield(driver.state(), "Dregscape Zombie").unwrap();
    let counters = driver
        .state()
        .object(id)
        .and_then(|object| object.permanent())
        .map(|held| held.counters_of(&CounterKind::PlusOnePlusOne));
    assert_eq!(counters, Some(1));
    assert_eq!(names(driver.state(), Zone::Hand(PlayerId(0))).len(), 1);
    replays(&driver, &setup);
    Ok(())
}

#[rstest]
fn t15a_worldly_tutor_puts_the_found_creature_on_top(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = t15a_table(seats)
        .hand(0, [card("Worldly Tutor").unwrap()])
        .library(
            0,
            [swamp(), card("Dregscape Zombie").unwrap(), swamp(), swamp()],
        )
        .battlefield(0, [ready(forest())]);
    let setup = scenario.clone().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario)?;
    driver.cast_tapping("Worldly Tutor", &["Forest"])?;
    t15a_settle_taking(&mut driver, "Dregscape Zombie")?;
    let library = names(driver.state(), Zone::Library(PlayerId(0)));
    assert_eq!(library.len(), 4);
    assert!(library.contains(&"Dregscape Zombie".to_string()));
    assert_eq!(
        library.last().map(String::as_str),
        Some("Dregscape Zombie"),
        "the creature is on top, the last card of the library zone"
    );
    replays(&driver, &setup);
    Ok(())
}
