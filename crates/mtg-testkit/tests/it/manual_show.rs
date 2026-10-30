//! Goal: a seat shows a card of its hand or its top card to every seat by hand and puts a short note on a card every seat sees, each as one action with its refusals by name; a shown card stays shown only while it is still that object in that place, a shuffle hides a shown top card, a note is trimmed, replaced and cleared, and a rebuilt engine reaches the same state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_core::ids::{ObjectId, PlayerId};
use mtg_engine::error::IllegalAnswer;
use mtg_engine::manual::{ManualAction, ManualRefusal, ManualReveal, ManualZone};
use mtg_engine::sight::{note_of, revealed_hand, revealed_top_by_hand};
use mtg_engine::zones::Zone;
use mtg_engine::{Engine, EngineError, Event};
use mtg_testkit::cards::{forest, free_creature};
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::{name_of, pid};

/// Seat 0 holds priority with a hand of A and B, a library of C over a forest and an Elf on the battlefield; seat 1 has
/// a hand and a library of its own.
fn game() -> Driver {
    Driver::from_scenario(
        Scenario::new(2)
            .hand(0, [free_creature("A", 1, 1), free_creature("B", 1, 1)])
            .library(0, [free_creature("C", 1, 1), forest()])
            .battlefield(0, [ready(free_creature("Elf", 1, 1))])
            .hand(1, [free_creature("Foe card", 1, 1)])
            .library(1, vec![forest(); 3])
            .battlefield(1, [ready(free_creature("Foe permanent", 2, 2))]),
    )
    .unwrap()
}

fn card(driver: &Driver, name: &str) -> ObjectId {
    driver
        .state()
        .objects()
        .find(|object| object.card().name == name)
        .unwrap_or_else(|| panic!("no {name}"))
        .id()
}

/// Takes the action as seat 0 and checks that a rebuilt engine reaches the same state.
fn take(driver: &mut Driver, action: ManualAction) -> Vec<Event> {
    let events = driver.manual(action).unwrap();
    let rebuilt = Engine::resume(
        driver.engine().setup().clone(),
        driver.engine().events().to_vec(),
    )
    .unwrap();
    assert_eq!(rebuilt.state_hash(), driver.state().state_hash());
    assert_eq!(
        revealed_hand(rebuilt.state(), pid(0)),
        revealed_hand(driver.state(), pid(0))
    );
    events
}

fn refusal(driver: &mut Driver, action: ManualAction) -> ManualRefusal {
    let hash = driver.state().state_hash();
    let error = driver.manual(action).unwrap_err();
    assert_eq!(driver.state().state_hash(), hash);
    let ScriptError::Engine(EngineError::Illegal(IllegalAnswer::Manual(refusal))) = error else {
        panic!("not a manual refusal: {error:?}");
    };
    refusal
}

fn reveal_hand(driver: &Driver, name: &str) -> ManualAction {
    ManualAction::Reveal {
        what: ManualReveal::Hand {
            object: card(driver, name),
        },
    }
}

fn note(driver: &Driver, name: &str, text: &str) -> ManualAction {
    ManualAction::Note {
        object: card(driver, name),
        text: text.to_owned(),
    }
}

fn every_seat_sees(driver: &Driver, id: ObjectId) -> bool {
    [pid(0), pid(1)]
        .into_iter()
        .all(|seat| driver.state().is_shown_to(seat, id))
}

#[test]
fn a_card_of_the_hand_is_shown_to_every_seat() {
    let mut driver = game();
    let a = card(&driver, "A");
    assert!(!driver.state().is_shown_to(pid(1), a));
    let action = reveal_hand(&driver, "A");
    let events = take(&mut driver, action);
    assert!(matches!(events[0], Event::ManualAction { .. }));
    assert!(
        matches!(&events[1], Event::Revealed(shown) if shown.cards == [a] && shown.player == pid(0))
    );
    assert_eq!(events.len(), 2);
    assert!(every_seat_sees(&driver, a));
    assert_eq!(revealed_hand(driver.state(), pid(0)), [a]);
    assert!(!driver.state().is_shown_to(pid(1), card(&driver, "B")));
}

#[test]
fn the_top_card_is_shown_to_every_seat_until_the_library_is_shuffled() {
    let mut driver = game();
    let c = card(&driver, "C");
    take(
        &mut driver,
        ManualAction::Reveal {
            what: ManualReveal::Top {},
        },
    );
    assert!(every_seat_sees(&driver, c));
    assert_eq!(revealed_top_by_hand(driver.state(), pid(0)), Some(c));
    assert_eq!(revealed_top_by_hand(driver.state(), pid(1)), None);
    take(&mut driver, ManualAction::Shuffle {});
    assert!(!driver.state().is_shown_to(pid(1), c));
    assert_eq!(revealed_top_by_hand(driver.state(), pid(0)), None);
}

#[test]
fn a_shown_card_is_hidden_again_once_it_changes_place() {
    let mut driver = game();
    let c = card(&driver, "C");
    take(
        &mut driver,
        ManualAction::Reveal {
            what: ManualReveal::Top {},
        },
    );
    take(&mut driver, ManualAction::Draw { count: 1 });
    let drawn = card(&driver, "C");
    assert_eq!(
        driver.state().object(drawn).unwrap().zone(),
        Zone::Hand(pid(0))
    );
    assert!(
        !driver.state().is_shown_to(pid(1), drawn),
        "{c:?} became a new object"
    );
    assert_eq!(
        revealed_hand(driver.state(), pid(0)),
        Vec::<ObjectId>::new()
    );
    let action = reveal_hand(&driver, "C");
    take(&mut driver, action);
    assert!(every_seat_sees(&driver, drawn));
}

#[rstest]
#[case::a_card_of_the_battlefield(Some("Elf"), ManualRefusal::WrongZone { object: ObjectId(0), zone: Zone::Battlefield })]
#[case::an_opponents_card(Some("Foe card"), ManualRefusal::NotYours)]
#[case::a_card_shown_already(None, ManualRefusal::NoChange)]
fn what_a_reveal_refuses(#[case] name: Option<&str>, #[case] expected: ManualRefusal) {
    let mut driver = game();
    if name.is_none() {
        let shown = reveal_hand(&driver, "A");
        take(&mut driver, shown);
    }
    let action = reveal_hand(&driver, name.unwrap_or("A"));
    let got = refusal(&mut driver, action);
    match (got, expected) {
        (ManualRefusal::WrongZone { zone, .. }, ManualRefusal::WrongZone { zone: want, .. }) => {
            assert_eq!(zone, want);
        }
        (got, expected) => assert_eq!(got, expected),
    }
}

#[test]
fn an_empty_library_has_no_top_card_to_show() {
    let mut driver = Driver::from_scenario(Scenario::new(2).library(0, Vec::new())).unwrap();
    let got = refusal(
        &mut driver,
        ManualAction::Reveal {
            what: ManualReveal::Top {},
        },
    );
    assert_eq!(got, ManualRefusal::NotEnough("cards in the library"));
}

#[test]
fn a_note_is_trimmed_replaced_and_cleared() {
    let mut driver = game();
    let elf = card(&driver, "Elf");
    let action = note(&driver, "Elf", "  has haste until end of turn ");
    let events = take(&mut driver, action);
    assert_eq!(events.len(), 1, "the action is the whole change");
    assert_eq!(
        note_of(driver.state(), elf),
        Some("has haste until end of turn")
    );
    let action = note(&driver, "Elf", "a 3/3 until end of turn");
    take(&mut driver, action);
    assert_eq!(
        note_of(driver.state(), elf),
        Some("a 3/3 until end of turn")
    );
    let action = note(&driver, "Elf", "   ");
    take(&mut driver, action);
    assert_eq!(note_of(driver.state(), elf), None);
}

#[test]
fn a_note_ends_when_the_card_becomes_another_object() {
    let mut driver = game();
    let elf = card(&driver, "Elf");
    let action = note(&driver, "Elf", "flickered");
    take(&mut driver, action);
    take(
        &mut driver,
        ManualAction::Move {
            object: elf,
            to: ManualZone::Graveyard,
        },
    );
    let dead = card(&driver, "Elf");
    assert_eq!(note_of(driver.state(), dead), None);
}

#[rstest]
#[case::a_card_in_the_hand("A", "x", ManualRefusal::WrongZone { object: ObjectId(0), zone: Zone::Hand(PlayerId(0)) })]
#[case::an_opponents_permanent("Foe permanent", "x", ManualRefusal::NotYours)]
#[case::no_note_to_clear("Elf", " ", ManualRefusal::NoChange)]
#[case::a_note_too_long("Elf", &"x".repeat(81), ManualRefusal::OutOfRange("the note"))]
#[case::a_note_with_a_line_break("Elf", "one\ntwo", ManualRefusal::OutOfRange("the note"))]
fn what_a_note_refuses(#[case] name: &str, #[case] text: &str, #[case] expected: ManualRefusal) {
    let mut driver = game();
    let action = note(&driver, name, text);
    let got = refusal(&mut driver, action);
    match (got, expected) {
        (ManualRefusal::WrongZone { zone, .. }, ManualRefusal::WrongZone { zone: want, .. }) => {
            assert_eq!(zone, want);
        }
        (got, expected) => assert_eq!(got, expected),
    }
}

#[test]
fn the_same_note_twice_changes_nothing() {
    let mut driver = game();
    let action = note(&driver, "Elf", "marked");
    take(&mut driver, action);
    let again = note(&driver, "Elf", " marked ");
    assert_eq!(refusal(&mut driver, again), ManualRefusal::NoChange);
}

#[test]
fn a_note_of_eighty_characters_is_kept() {
    let mut driver = game();
    let text = "y".repeat(80);
    let action = note(&driver, "Elf", &text);
    take(&mut driver, action);
    assert_eq!(
        note_of(driver.state(), card(&driver, "Elf")),
        Some(text.as_str())
    );
    assert_eq!(name_of(driver.state(), card(&driver, "Elf")), "Elf");
}
