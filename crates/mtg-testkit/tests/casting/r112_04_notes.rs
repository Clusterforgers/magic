//! Goal: Long List of the Ents notes a creature type as each chapter resolves (`Effect::R112NoteChosen`), never offers a
//! type it already noted, and the next creature spell of the noted type cast that turn enters with an additional
//! +1/+1 counter while a spell of another type does not, at tables of two to four seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::value::CounterKind;
use mtg_core::card::Subtype;
use mtg_core::color::Color;
use mtg_engine::choose::{ChoiceOption, OptionChoice};
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{creature, forest};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario};
use rstest::rstest;

use crate::common::casting::lands;
use crate::common::pid;

/// A 1/1 Elf for {G}.
fn elf() -> Arc<CardDefinition> {
    let mut elf = (*creature("Elvish Scout", 0, &[Color::Green], 1, 1)).clone();
    elf.subtypes = vec![Subtype("Elf".to_owned())];
    Arc::new(elf)
}

/// The creature types offered by the choice asked now, if it is the Saga's note.
fn offered(driver: &Driver) -> Option<Vec<String>> {
    let decision = driver.decision().ok()?;
    let DecisionKind::ChooseOption(options) = &decision.kind else {
        return None;
    };
    let labels: Vec<String> = options
        .options
        .iter()
        .filter_map(|option| match option {
            ChoiceOption::Label(name) => Some(name.clone()),
            _ => None,
        })
        .collect();
    (labels.len() > 100).then_some(labels)
}

/// Answers passively until the Saga asks for a type to note; the types offered.
fn until_note(driver: &mut Driver) -> Vec<String> {
    for _ in 0..400 {
        if let Some(types) = offered(driver) {
            return types;
        }
        driver.answer_passively().unwrap();
    }
    panic!("the Saga never asked for a creature type");
}

/// Answers passively until the stack is empty and nothing resolves.
fn settle(driver: &mut Driver) {
    for _ in 0..200 {
        if driver.state().stack().is_empty() && driver.state().resolving().is_none() {
            return;
        }
        driver.answer_passively().unwrap();
    }
    panic!("the stack never emptied");
}

/// Seat 0 casts the Saga with a Forest and notes `noted` as chapter I resolves.
fn noted(seats: usize, noted: &str) -> Driver {
    let scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .hand(0, [card("Long List of the Ents").unwrap(), elf()])
        .battlefield(0, lands(2, forest));
    let mut driver = Driver::from_scenario(scenario).unwrap();
    driver.cast("Long List of the Ents").unwrap();
    let types = until_note(&mut driver);
    assert_eq!(driver.decision().unwrap().player, pid(0));
    let pick = types.iter().position(|name| name == noted).unwrap();
    driver
        .answer(Answer::ChooseOption(OptionChoice(pick)))
        .unwrap();
    settle(&mut driver);
    driver
}

#[rstest]
fn r112_the_next_creature_spell_of_the_noted_type_enters_with_a_counter(
    #[values(2, 3, 4)] seats: usize,
    #[values("Elf", "Goblin")] type_noted: &str,
) {
    let mut driver = noted(seats, type_noted);
    driver.cast_tapping("Elvish Scout", &["Forest"]).unwrap();
    settle(&mut driver);
    let state = driver.state();
    let scout = state
        .objects()
        .find(|object| object.card().name == "Elvish Scout")
        .unwrap();
    assert_eq!(scout.zone(), Zone::Battlefield);
    let counters = scout
        .permanent()
        .unwrap()
        .counters_of(&CounterKind::PlusOnePlusOne);
    assert_eq!(counters, u32::from(type_noted == "Elf"));
}

#[rstest]
fn r112_a_type_noted_for_the_saga_is_not_offered_again(#[values(2, 3, 4)] seats: usize) {
    let mut driver = noted(seats, "Elf");
    let next = u32::try_from(seats).unwrap() + 1;
    driver.idle_until(next, Step::Upkeep).unwrap();
    let types = until_note(&mut driver);
    assert!(!types.iter().any(|name| name == "Elf"));
    assert!(types.iter().any(|name| name == "Goblin"));
}
