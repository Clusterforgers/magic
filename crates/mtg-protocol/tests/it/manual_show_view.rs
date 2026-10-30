//! Goal: a card a seat shows by hand is named to every seat and spectator, in the views and the batches, as a hand card in `revealed_hand` or the top card in `revealed_top`, while the cards beside it stay hidden; a shuffle hides the top card again; a note on a card is read by every seat that sees the card; the views the seats fold equal the true ones.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_core::ids::{ObjectId, PlayerId};
use mtg_engine::manual::{ManualAction, ManualReveal};
use mtg_protocol::{EventBatch, StateView};
use mtg_testkit::cards::{forest, free_creature};
use mtg_testkit::{Scenario, ready};
use rstest::rstest;
use serde_json::Value;

use crate::common::table::{Run, Table, seats};

const SHOWN: &str = "Shown card";
const KEPT: &str = "Kept card";
const TOP: &str = "Top secret";
const BELOW: &str = "Below secret";

/// Seat 0 has two cards in hand, a library of two named cards over forests and a creature in play.
fn table(seat_count: usize) -> Table {
    let mut scenario = Scenario::new(seat_count)
        .hand(0, [free_creature(SHOWN, 1, 1), free_creature(KEPT, 1, 1)])
        .library(
            0,
            [
                free_creature(TOP, 1, 1),
                free_creature(BELOW, 1, 1),
                forest(),
                forest(),
            ],
        )
        .battlefield(0, [ready(free_creature("Elf", 1, 1))]);
    for other in 1..seat_count {
        scenario = scenario.library(other, vec![forest(); 4]);
    }
    Table::new(scenario)
}

fn act(table: &mut Table, action: ManualAction) -> Run {
    table.run(|driver| driver.manual(action))
}

fn names(value: &Value, name: &str) -> bool {
    match value {
        Value::String(text) => text == name,
        Value::Array(items) => items.iter().any(|item| names(item, name)),
        Value::Object(map) => map.values().any(|item| names(item, name)),
        _ => false,
    }
}

fn batch_names(batch: &EventBatch, name: &str) -> bool {
    [
        serde_json::to_value(&batch.events).unwrap(),
        serde_json::to_value(&batch.table).unwrap(),
        serde_json::to_value(&batch.objects).unwrap(),
    ]
    .iter()
    .any(|part| names(part, name))
}

fn view_names(view: &StateView, name: &str) -> bool {
    names(&serde_json::to_value(view).unwrap(), name)
}

#[rstest]
fn a_card_shown_from_the_hand_is_named_to_every_seat(#[values(2, 3, 4)] seat_count: usize) {
    let mut table = table(seat_count);
    let shown = card(&table, SHOWN);
    let run = act(
        &mut table,
        ManualAction::Reveal {
            what: ManualReveal::Hand { object: shown },
        },
    );
    for (viewer, batch) in seats(seat_count).into_iter().zip(&run.batches) {
        if viewer != PlayerId(0) {
            assert!(batch_names(batch, SHOWN), "{viewer:?} is told the card");
        }
        if viewer != PlayerId(0) {
            assert!(!batch_names(batch, KEPT), "{viewer:?}");
        }
        assert!(
            !batch_names(batch, TOP),
            "{viewer:?} learns nothing of the library"
        );
        let view = StateView::for_player(table.driver.state(), viewer);
        assert_eq!(view.table.players[0].revealed_hand, [shown], "{viewer:?}");
        assert!(view.objects.iter().any(|object| object.id == shown));
    }
    let spectator = StateView::for_spectator(table.driver.state());
    assert_eq!(spectator.table.players[0].revealed_hand, [shown]);
    assert!(view_names(&spectator, SHOWN));
    assert!(!view_names(&spectator, KEPT));
}

fn card(table: &Table, name: &str) -> ObjectId {
    table
        .driver
        .state()
        .objects()
        .find(|o| o.card().name == name)
        .unwrap_or_else(|| panic!("no {name}"))
        .id()
}

#[rstest]
fn the_top_card_shown_is_named_to_every_seat_and_the_cards_under_it_are_not(
    #[values(2, 3)] seat_count: usize,
) {
    let mut table = table(seat_count);
    let run = act(
        &mut table,
        ManualAction::Reveal {
            what: ManualReveal::Top {},
        },
    );
    let top = card(&table, TOP);
    for (viewer, batch) in seats(seat_count).into_iter().zip(&run.batches) {
        if viewer != PlayerId(0) {
            assert!(batch_names(batch, TOP), "{viewer:?} is told the card");
        }
        assert!(
            !batch_names(batch, BELOW),
            "{viewer:?} is not told the next card"
        );
        let view = StateView::for_player(table.driver.state(), viewer);
        assert_eq!(view.table.players[0].revealed_top, Some(top), "{viewer:?}");
        let known = if viewer == PlayerId(0) {
            vec![top]
        } else {
            Vec::new()
        };
        assert_eq!(view.table.players[0].known_top, known, "{viewer:?}");
    }
    let spectator = StateView::for_spectator(table.driver.state());
    assert_eq!(spectator.table.players[0].revealed_top, Some(top));
    assert!(!view_names(&spectator, BELOW));
}

#[test]
fn a_shuffle_hides_the_top_card_again() {
    let mut table = table(2);
    act(
        &mut table,
        ManualAction::Reveal {
            what: ManualReveal::Top {},
        },
    );
    let run = act(&mut table, ManualAction::Shuffle {});
    for viewer in seats(2) {
        let view = StateView::for_player(table.driver.state(), viewer);
        assert_eq!(view.table.players[0].revealed_top, None, "{viewer:?}");
        assert!(!view_names(&view, TOP), "{viewer:?}");
    }
    assert!(!batch_names(&run.batches[1], TOP));
}

#[rstest]
fn a_note_is_read_by_every_seat_that_sees_the_card(#[values(2, 4)] seat_count: usize) {
    let mut table = table(seat_count);
    let elf = card(&table, "Elf");
    let run = act(
        &mut table,
        ManualAction::Note {
            object: elf,
            text: "has haste".to_owned(),
        },
    );
    for (viewer, batch) in seats(seat_count).into_iter().zip(&run.batches) {
        let told = batch.objects.iter().find(|object| object.id == elf);
        assert_eq!(
            told.and_then(|object| object.note.as_deref()),
            Some("has haste"),
            "{viewer:?}"
        );
    }
    let spectator = StateView::for_spectator(table.driver.state());
    let seen = spectator.objects.iter().find(|object| object.id == elf);
    assert_eq!(
        seen.and_then(|object| object.note.as_deref()),
        Some("has haste")
    );
    let run = act(
        &mut table,
        ManualAction::Note {
            object: elf,
            text: String::new(),
        },
    );
    let cleared = run.batches[1]
        .objects
        .iter()
        .find(|object| object.id == elf);
    assert_eq!(cleared.map(|object| object.note.clone()), Some(None));
}
