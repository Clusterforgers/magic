//! Goal: Reed Richards, Smartest Man replaces only the first time its controller would draw a card each turn, at tables
//! of two to four seats: on its controller's turn the draw step's first card is drawn as usual and the next draw becomes
//! four, a draw of two becomes four and one, a later draw stays one; on another player's turn the first draw becomes four.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::ability::Ability;
use mtg_cards::definition::CardDefinition;
use mtg_core::card::CardType;
use mtg_engine::decision::DecisionKind;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_engine::{GameSetup, replay};
use mtg_testkit::cards::free_creature;
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::pid;

fn replays(driver: &Driver, setup: &GameSetup) {
    let replayed = replay(setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
}

/// A free instant: "Draw `count` cards."
fn r109_drawer(name: &str, count: u32) -> Arc<CardDefinition> {
    let ability: Ability =
        serde_json::from_str(&format!(r#"{{"Spell":{{"effects":[{{"Draw":{{"count":{count}}}}}]}}}}"#))
            .unwrap();
    Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Instant)
            .with_rules(ability),
    )
}

/// Seat 0 controls Reed and holds a draw of two and a draw of one; every library holds twenty creatures.
fn r109_table(seats: usize, active: usize, step: Step) -> Scenario {
    (0..seats).fold(
        Scenario::new(seats)
            .at(2, active, step)
            .battlefield(0, [ready(card("Reed Richards, Smartest Man").unwrap())])
            .hand(0, [r109_drawer("Draw Two", 2), r109_drawer("Draw One", 1)]),
        |scenario, seat| {
            scenario.library(
                seat,
                (0..20).map(|i| free_creature(&format!("Pad {seat} {i}"), 1, 1)),
            )
        },
    )
}

/// Casts the named instant as seat 0 and passes until it has resolved.
fn r109_cast_and_resolve(driver: &mut Driver, name: &str) -> Result<(), ScriptError> {
    while driver.decision()?.player != pid(0) {
        driver.pass()?;
    }
    driver.cast(name)?;
    for _ in 0..40 {
        let decision = driver.decision()?.clone();
        match decision.kind {
            DecisionKind::Priority(_) if driver.state().stack().is_empty() => return Ok(()),
            DecisionKind::Priority(_) => {
                driver.pass()?;
            }
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    Ok(())
}

fn library(driver: &Driver) -> usize {
    driver.state().zone(Zone::Library(pid(0))).len()
}

#[rstest]
fn r109_reed_on_its_controllers_turn(#[values(2, 3, 4)] seats: usize) -> Result<(), ScriptError> {
    let setup = r109_table(seats, 0, Step::Upkeep).setup().unwrap();
    let mut driver = Driver::from_scenario(r109_table(seats, 0, Step::Upkeep))?;
    driver.pass_until(Step::Main1)?;
    assert_eq!(library(&driver), 19, "the draw step's card is drawn as usual");
    r109_cast_and_resolve(&mut driver, "Draw Two")?;
    assert_eq!(library(&driver), 14, "four instead of the first, then one");
    r109_cast_and_resolve(&mut driver, "Draw One")?;
    assert_eq!(library(&driver), 13, "a later draw is one card");
    replays(&driver, &setup);
    Ok(())
}

#[rstest]
fn r109_reed_on_another_players_turn(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let active = seats - 1;
    let setup = r109_table(seats, active, Step::Main1).setup().unwrap();
    let mut driver = Driver::from_scenario(r109_table(seats, active, Step::Main1))?;
    r109_cast_and_resolve(&mut driver, "Draw One")?;
    assert_eq!(library(&driver), 16, "the first draw of the turn is four");
    r109_cast_and_resolve(&mut driver, "Draw Two")?;
    assert_eq!(library(&driver), 14);
    replays(&driver, &setup);
    Ok(())
}
