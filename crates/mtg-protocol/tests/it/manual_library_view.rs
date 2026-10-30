//! Goal: the top cards a seat looks at by hand are named to that seat alone, in its view as `known_top` and in its batch, and never to another seat or a spectator; putting them back tells the others only how many went where, a card put into the graveyard is named to all, and a shuffle makes the seat forget them; the views the seats fold equal the true ones.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_core::ids::{ObjectId, PlayerId};
use mtg_engine::Event;
use mtg_engine::manual::ManualAction;
use mtg_protocol::{EventBatch, EventView, HiddenEvent, StateView};
use mtg_testkit::Scenario;
use mtg_testkit::cards::{forest, free_creature};
use rstest::rstest;
use serde_json::Value;

use crate::common::table::{Run, Table, seats};
use crate::hidden_info::{assert_clean, hidden_in};

const TOP: &str = "Secret top";
const NEXT: &str = "Secret next";

/// Seat 0 has a library of two named cards over two forests; every other seat has a library of its own.
fn table(seat_count: usize) -> Table {
    let mut scenario = Scenario::new(seat_count).library(
        0,
        [
            free_creature(TOP, 1, 1),
            free_creature(NEXT, 1, 1),
            forest(),
            forest(),
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

/// What the seat's own view lists as the top cards it knows.
fn known_top(table: &Table, seat: PlayerId) -> Vec<ObjectId> {
    let view = StateView::for_player(table.driver.state(), seat);
    view.table.players[0].known_top.clone()
}

#[rstest]
fn a_look_at_the_top_is_the_seats_alone(#[values(2, 3, 4)] seat_count: usize) {
    let mut table = table(seat_count);
    let run = act(&mut table, ManualAction::LookTop { count: 2 });
    let after = table.driver.state().clone();
    for (viewer, batch) in seats(seat_count).into_iter().zip(&run.batches) {
        let owner = viewer == PlayerId(0);
        assert_eq!(batch_names(batch, TOP), owner, "{viewer:?}");
        assert_eq!(batch_names(batch, NEXT), owner, "{viewer:?}");
        if !owner {
            assert_clean(batch, &hidden_in(&after, viewer));
            assert_eq!(known_top(&table, viewer), Vec::<ObjectId>::new());
        }
    }
    assert_eq!(
        known_top(&table, PlayerId(0)),
        [card(&table, TOP), card(&table, NEXT)]
    );
    let spectator = StateView::for_spectator(table.driver.state());
    assert_eq!(spectator.table.players[0].known_top, Vec::<ObjectId>::new());
    assert!(!names(&serde_json::to_value(&spectator).unwrap(), TOP));
}

#[rstest]
#[case::reorder(&[NEXT, TOP], &[], &[], (2, 0, 0))]
#[case::scry_one_to_the_bottom(&[NEXT], &[TOP], &[], (1, 1, 0))]
#[case::surveil_one_away(&[NEXT], &[], &[TOP], (1, 0, 1))]
#[case::all_three_ways(&[], &[NEXT], &[TOP], (0, 1, 1))]
fn putting_the_cards_back_tells_the_others_only_how_many(
    #[values(2, 3)] seat_count: usize,
    #[case] top: &[&str],
    #[case] bottom: &[&str],
    #[case] graveyard: &[&str],
    #[case] counts: (u32, u32, u32),
) {
    let mut table = table(seat_count);
    act(&mut table, ManualAction::LookTop { count: 2 });
    let ids =
        |table: &Table, names: &[&str]| names.iter().map(|n| card(table, n)).collect::<Vec<_>>();
    let (top_ids, bottom_ids) = (ids(&table, top), ids(&table, bottom));
    let action = ManualAction::ArrangeTop {
        top: top_ids.clone(),
        bottom: bottom_ids.clone(),
        graveyard: ids(&table, graveyard),
    };
    let run = act(&mut table, action.clone());
    let after = table.driver.state().clone();
    for (viewer, batch) in seats(seat_count).into_iter().zip(&run.batches) {
        match &batch.events[0] {
            EventView::Public(Event::ManualAction { action: told, .. }) => {
                assert_eq!(viewer, PlayerId(0));
                assert_eq!(**told, action);
            }
            EventView::Hidden(HiddenEvent::ManualArrange {
                player,
                top,
                bottom,
                graveyard,
            }) => {
                assert_ne!(viewer, PlayerId(0));
                assert_eq!(
                    (*player, (*top, *bottom, *graveyard)),
                    (PlayerId(0), counts)
                );
                // The counts are small numbers, which the check would take for the ids of the first cards; those are
                // checked by name.
                let hidden = hidden_in(&after, viewer)
                    .into_iter()
                    .filter(|(id, _)| id.0 > 2)
                    .collect();
                assert_clean(batch, &hidden);
                assert!(!batch_names(batch, NEXT));
                let milled = graveyard_names(*graveyard);
                assert_eq!(batch_names(batch, TOP), milled.contains(&TOP));
            }
            other => panic!("the first event is the action, not {other:?}"),
        }
    }
    let library = after.zone(mtg_engine::zones::Zone::Library(PlayerId(0)));
    let top_down: Vec<ObjectId> = library.iter().rev().copied().collect();
    assert_eq!(top_down[..top.len()], top_ids[..]);
    assert_eq!(top_down[top_down.len() - bottom.len()..], bottom_ids[..]);
}

/// The names of the cards a case puts into the graveyard, by count: only the top card is ever milled.
fn graveyard_names(count: u32) -> Vec<&'static str> {
    if count == 0 { Vec::new() } else { vec![TOP] }
}

#[test]
fn a_shuffle_makes_the_seat_forget_what_it_looked_at() {
    let mut table = table(2);
    act(&mut table, ManualAction::LookTop { count: 2 });
    let run = act(&mut table, ManualAction::Shuffle {});
    assert_eq!(known_top(&table, PlayerId(0)), Vec::<ObjectId>::new());
    assert!(matches!(
        run.batches[1].events[1],
        EventView::Hidden(HiddenEvent::LibraryShuffled { .. })
    ));
}
