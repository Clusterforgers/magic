//! Goal: The Apprentice's Folly, at tables of two to four: chapters I and II each make a token copy of the target
//! nontoken creature its controller controls that isn't legendary, is a Reflection besides its other types and has
//! haste; a creature with the same name as a token its controller controls can't be the target; chapter III sacrifices
//! every Reflection its controller controls and nothing else.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::keyword::Keyword;
use mtg_core::card::{Subtype, Supertype};
use mtg_core::ids::{ObjectId, PlayerId};
use mtg_core::mana::ManaCost;
use mtg_engine::characteristics::characteristics;
use mtg_engine::choose::Target;
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::keywords::has_keyword;
use mtg_engine::object::ObjectKind;
use mtg_engine::zones::Zone;
use mtg_testkit::attach_cards::legend;
use mtg_testkit::cards::{bear, swamp};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ready};
use rstest::rstest;

const FOLLY: &str = "The Apprentice's Folly";

/// A corpus card for no mana.
fn free(name: &str) -> Arc<CardDefinition> {
    let mut definition = (*card(name).unwrap()).clone();
    definition.mana_cost = Some(ManaCost::parse("{0}").unwrap());
    Arc::new(definition)
}

/// The tokens seat 0 controls.
fn tokens(driver: &Driver) -> Vec<ObjectId> {
    driver
        .state()
        .zone(Zone::Battlefield)
        .iter()
        .copied()
        .filter(|id| {
            let object = driver.state().object(*id).unwrap();
            object.kind() == ObjectKind::Token && object.controller() == PlayerId(0)
        })
        .collect()
}

/// Answers until the stack is empty at seat 0's priority, targeting `pick` (by name) when a chapter asks; records the
/// names the chapter offered.
fn resolve(driver: &mut Driver, pick: &str, offered: &mut Vec<Vec<String>>) {
    for _ in 0..200 {
        let decision = driver.decision().unwrap().clone();
        match &decision.kind {
            DecisionKind::ChooseTargets(options) => {
                let names = options
                    .legal
                    .iter()
                    .filter_map(|target| match target {
                        Target::Object(reference) => driver
                            .state()
                            .object(reference.id)
                            .map(|object| object.card().name.clone()),
                        Target::Player(_) => None,
                    })
                    .collect();
                offered.push(names);
                let target = driver.target_named(pick).unwrap();
                driver.choose_targets(&[target]).unwrap();
            }
            DecisionKind::PayMana(_) => {
                driver.answer(Answer::PayMana(PayChoice::Auto)).unwrap();
            }
            DecisionKind::Priority(_)
                if driver.state().stack().is_empty() && decision.player == PlayerId(0) =>
            {
                return;
            }
            _ => {
                driver.answer_passively().unwrap();
            }
        }
    }
    panic!("the stack does not empty");
}

/// Passes until seat 0's next first main phase, answering what comes up on the way like `resolve`.
fn next_turn(driver: &mut Driver, pick: &str, offered: &mut Vec<Vec<String>>) {
    let turn = driver.state().turn().number();
    for _ in 0..2000 {
        let state = driver.state();
        if state.turn().number() > turn
            && state.turn().active() == PlayerId(0)
            && state.turn().step() == mtg_engine::turn::Step::Main1
        {
            resolve(driver, pick, offered);
            return;
        }
        if matches!(
            driver.decision().unwrap().kind,
            DecisionKind::ChooseTargets(_) | DecisionKind::PayMana(_)
        ) {
            resolve(driver, pick, offered);
        } else {
            driver.answer_passively().unwrap();
        }
    }
    panic!("seat 0's turn does not come");
}

#[rstest]
fn the_folly_copies_twice_and_sacrifices_the_reflections(#[values(2, 3, 4)] seats: usize) {
    let mut scenario = Scenario::new(seats)
        .hand(0, [free(FOLLY)])
        .battlefield(0, [ready(legend("Hero", (1, &[]), 2, 2)), ready(bear())]);
    for seat in 0..seats {
        scenario = scenario.library(seat, (0..20).map(|_| swamp()));
    }
    let mut driver = Driver::from_scenario(scenario).unwrap();
    let mut offered = Vec::new();
    driver.begin_cast(FOLLY).unwrap();
    resolve(&mut driver, "Hero", &mut offered);
    // Chapter I: both creatures may be chosen; the copy of Hero is a hasty Reflection and not legendary.
    assert_eq!(offered.len(), 1);
    assert_eq!(offered[0].len(), 2, "{offered:?}");
    let made = tokens(&driver);
    assert_eq!(made.len(), 1);
    let copy = characteristics(driver.state(), made[0]).unwrap();
    assert!(!copy.supertypes.contains(&Supertype::Legendary));
    let token = driver.state().object(made[0]).unwrap();
    assert!(copy.has_subtype(&token.card().subtypes, &Subtype("Reflection".to_owned())));
    assert!(has_keyword(driver.state(), made[0], &Keyword::Haste));
    // Chapter II: Hero shares its name with a token, so only the other creature is offered.
    next_turn(&mut driver, bear().name.as_str(), &mut offered);
    assert_eq!(offered.len(), 2);
    assert_eq!(offered[1], vec![bear().name.clone()], "{offered:?}");
    assert_eq!(tokens(&driver).len(), 2);
    // Chapter III: both Reflections are sacrificed, the nontoken creatures stay.
    next_turn(&mut driver, "Hero", &mut offered);
    assert!(tokens(&driver).is_empty());
    assert!(driver.id_of("Hero").is_ok());
    assert!(driver.id_of(&bear().name).is_ok());
}
