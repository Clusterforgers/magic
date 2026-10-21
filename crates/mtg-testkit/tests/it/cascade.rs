//! Goal: cascade plays whole through Bloodbraid Elf and a cascade creature of the tests: the trigger exiles from the top until a nonland card of lesser mana value (each exiled card is open to every seat), the controller may cast it without paying its mana cost with every question of the cast asked inside the resolution, the spell goes on the stack above the trigger and resolves first, and every exiled card not cast goes to the bottom in a random order that the seed fixes; a library without a card of lesser value, a card that cannot be cast (no legal target) and a declined cast leave all cards at the bottom, an X spell is cast with X 0, a modal spell asks its modes, a cost increase is still paid, and a cascade card found by a cascade cascades in its turn.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::keyword::Keyword;
use mtg_engine::Event;
use mtg_engine::choose::YesNoChoice;
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{creature, forest, keyworded_creature, mountain};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};

use crate::common::casting::resolve;
use crate::common::{names, pid};

/// Seat 0 holds Bloodbraid Elf over two Mountains and two Forests; its library is `library`, top first.
fn elf_game(library: Vec<Arc<CardDefinition>>) -> Scenario {
    let lands = [mountain(), mountain(), forest(), forest()].map(ready);
    Scenario::new(2)
        .hand_named(0, ["Bloodbraid Elf"])
        .battlefield(0, lands)
        .library(0, library)
}

fn bear(name: &str, generic: u32) -> Arc<CardDefinition> {
    creature(name, generic, &[], 2, 2)
}

/// Casts the Elf and lets the cascade trigger resolve as far as its first question.
fn cascade(driver: &mut Driver) -> Result<(), ScriptError> {
    driver.cast("Bloodbraid Elf")?;
    resolve(driver)
}

fn say(driver: &mut Driver, yes: bool) -> Result<Vec<Event>, ScriptError> {
    driver.answer(Answer::YesNo(YesNoChoice(yes)))
}

/// The names in the library, top card first.
fn library_top_first(driver: &Driver, seat: usize) -> Vec<String> {
    let mut library = names(driver.state(), Zone::Library(pid(seat)));
    library.reverse();
    library
}

#[test]
fn the_first_card_of_lesser_value_is_cast_for_nothing_and_the_rest_go_to_the_bottom()
-> Result<(), ScriptError> {
    let library = vec![
        forest(),
        mountain(),
        bear("Three", 3),
        bear("Four", 4),
        bear("Deep", 1),
    ];
    let mut driver = Driver::from_scenario(elf_game(library))?;
    cascade(&mut driver)?;
    let DecisionKind::YesNo(question) = &driver.decision()?.kind else {
        panic!("the cast is asked: {:?}", driver.decision()?.kind.name());
    };
    assert_eq!(question.text, "Cast Three without paying its mana cost?");
    assert_eq!(
        names(driver.state(), Zone::Exile),
        ["Forest", "Mountain", "Three"],
        "every card exiled is open on the table"
    );
    let events = say(&mut driver, true)?;
    assert!(events.iter().any(|event| matches!(event, Event::SpellCast { .. })));
    assert_eq!(
        names(driver.state(), Zone::Stack),
        ["Bloodbraid Elf", "Three"],
        "the spell is above the elf, whose trigger is gone"
    );
    assert!(names(driver.state(), Zone::Exile).is_empty());
    let library = library_top_first(&driver, 0);
    assert_eq!(library[..2], ["Four", "Deep"]);
    let mut bottom = library[2..].to_vec();
    bottom.sort();
    assert_eq!(bottom, ["Forest", "Mountain"]);
    resolve(&mut driver)?;
    assert_eq!(names(driver.state(), Zone::Stack), ["Bloodbraid Elf"]);
    assert!(names(driver.state(), Zone::Battlefield).contains(&"Three".to_owned()));
    Ok(())
}

