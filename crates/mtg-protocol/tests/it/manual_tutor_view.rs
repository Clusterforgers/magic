//! Goal: a search of the library by hand names its cards to the searching seat alone, a card it puts into its hand from the library is told to every other seat and a spectator only as `HiddenEvent::ManualTutor` with the library shuffled, and a sacrifice, a summoning sickness removed and a control given by hand are public as logged; the views the seats fold equal the true ones.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_core::ids::{ObjectId, PlayerId};
use mtg_engine::Event;
use mtg_engine::manual::ManualAction;
use mtg_protocol::{EventBatch, EventView, HiddenEvent, StateView};
use mtg_testkit::cards::{forest, free_creature};
use mtg_testkit::{Scenario, ready, sick};
use rstest::rstest;
use serde_json::Value;

use crate::common::table::{Run, Table, seats};
use crate::hidden_info::{assert_clean, hidden_in};

const WANTED: &str = "Secret wanted";
const OTHER: &str = "Secret other";

/// Seat 0 has a library of two named cards over forests and two creatures in play; every other seat has a library.
fn table(seat_count: usize) -> Table {
    let mut scenario = Scenario::new(seat_count)
        .library(
            0,
            [
                forest(),
                free_creature(WANTED, 1, 1),
                free_creature(OTHER, 1, 1),
                forest(),
            ],
        )
        .battlefield(
            0,
            [
                ready(free_creature("Mine", 2, 2)),
                sick(free_creature("Fresh", 1, 1)),
            ],
        );
    for other in 1..seat_count {
        scenario = scenario.library(other, vec![forest(); 4]);
    }
    Table::new(scenario)
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

#[rstest]
fn a_search_is_the_seats_alone(#[values(2, 3, 4)] seat_count: usize) {
    let mut table = table(seat_count);
    let run = act(&mut table, ManualAction::Search {});
    let after = table.driver.state().clone();
    for (viewer, batch) in seats(seat_count).into_iter().zip(&run.batches) {
        let owner = viewer == PlayerId(0);
        assert_eq!(batch_names(batch, WANTED), owner, "{viewer:?}");
        assert_eq!(batch_names(batch, OTHER), owner, "{viewer:?}");
        if !owner {
            assert_clean(batch, &hidden_in(&after, viewer));
        }
    }
    let own = StateView::for_player(table.driver.state(), PlayerId(0));
    assert_eq!(own.table.players[0].known_top.len(), 4);
    let spectator = StateView::for_spectator(table.driver.state());
    assert_eq!(spectator.table.players[0].known_top, Vec::<ObjectId>::new());
    assert!(!names(&serde_json::to_value(&spectator).unwrap(), WANTED));
}

#[rstest]
fn a_tutored_card_is_withheld_from_the_others(#[values(2, 3, 4)] seat_count: usize) {
    let mut table = table(seat_count);
    act(&mut table, ManualAction::Search {});
    let wanted = card(&table, WANTED);
    let action = ManualAction::Tutor { object: wanted };
    let run = act(&mut table, action.clone());
    let after = table.driver.state().clone();
    for (viewer, batch) in seats(seat_count).into_iter().zip(&run.batches) {
        match &batch.events[0] {
            EventView::Public(Event::ManualAction { action: told, .. }) => {
                assert_eq!(viewer, PlayerId(0));
                assert_eq!(**told, action);
            }
            EventView::Hidden(HiddenEvent::ManualTutor { player }) => {
                assert_ne!(viewer, PlayerId(0));
                assert_eq!(*player, PlayerId(0));
                assert!(!batch_names(batch, WANTED));
                assert!(!batch_names(batch, OTHER));
                assert_clean(batch, &hidden_in(&after, viewer));
            }
            other => panic!("the first event is the action, not {other:?}"),
        }
        assert!(
            batch.events.iter().any(|event| matches!(
                event,
                EventView::Hidden(HiddenEvent::LibraryShuffled { .. })
            ) || matches!(
                event,
                EventView::Public(Event::LibraryShuffled { .. })
            )),
            "{viewer:?} is told of the shuffle"
        );
    }
    let own = StateView::for_player(table.driver.state(), PlayerId(0));
    assert_eq!(own.table.players[0].known_top, Vec::<ObjectId>::new());
    let spectator = StateView::for_spectator(table.driver.state());
    let shown = serde_json::to_value(&spectator).unwrap();
    assert!(!names(&shown, WANTED) && !names(&shown, OTHER));
}

#[rstest]
#[case::a_sacrifice(|t: &Table| ManualAction::Sacrifice { object: card(t, "Mine") })]
#[case::a_sickness_removed(|t: &Table| ManualAction::RemoveSickness { object: card(t, "Fresh") })]
#[case::control_given(|t: &Table| ManualAction::GiveControl { object: card(t, "Mine"), to: PlayerId(1) })]
fn a_change_on_the_battlefield_is_public(
    #[values(2, 3, 4)] seat_count: usize,
    #[case] action: fn(&Table) -> ManualAction,
) {
    let mut table = table(seat_count);
    let action = action(&table);
    let run = act(&mut table, action.clone());
    for batch in &run.batches {
        assert_eq!(
            batch.events[0],
            EventView::Public(Event::ManualAction {
                player: PlayerId(0),
                action: Box::new(action.clone()),
            })
        );
    }
}
