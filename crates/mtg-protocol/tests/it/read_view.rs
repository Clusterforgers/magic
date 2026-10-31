//! Goal: an object whose card runs on a player's read says so to every seat that sees its face, a card in a hand shows it to its owner alone, an object of any other card leaves the member out of its JSON, and the member survives the wire.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_protocol::ObjectView;
use mtg_testkit::cards::{forest, free_creature};
use mtg_testkit::{Scenario, ready};
use rstest::rstest;

use crate::common::table::Table;

/// A 2/2 whose rules are a draft a player approved.
fn read_creature(name: &str) -> Arc<CardDefinition> {
    let mut definition = (*free_creature(name, 2, 2)).clone();
    definition.read = true;
    Arc::new(definition)
}

fn table(seat_count: usize) -> Table {
    let mut scenario = Scenario::new(seat_count)
        .hand(0, [read_creature("Secret reader")])
        .library(0, vec![forest(); 4])
        .battlefield(
            0,
            [
                ready(read_creature("Open reader")),
                ready(free_creature("Open bear", 2, 2)),
            ],
        );
    for other in 1..seat_count {
        scenario = scenario.library(other, vec![forest(); 4]);
    }
    Table::new(scenario)
}

fn named<'a>(table: &'a Table, seat: usize, name: &str) -> Option<&'a ObjectView> {
    table.views[seat].objects.iter().find(|o| o.name == name)
}

#[rstest]
fn a_card_that_runs_on_a_read_says_so_to_every_seat_that_sees_it(
    #[values(2, 3, 4)] seat_count: usize,
) {
    let table = table(seat_count);
    for seat in 0..seat_count {
        let reader = named(&table, seat, "Open reader").unwrap();
        assert!(reader.read, "seat {seat}");
        let json = serde_json::to_value(reader).unwrap();
        assert_eq!(json["read"], serde_json::json!(true));
        let bear = named(&table, seat, "Open bear").unwrap();
        assert!(!bear.read, "seat {seat}");
        assert!(
            serde_json::to_value(bear).unwrap().get("read").is_none(),
            "left out while false"
        );
    }
}

#[rstest]
fn a_read_card_in_a_hand_is_shown_to_its_owner_alone(#[values(2, 3, 4)] seat_count: usize) {
    let table = table(seat_count);
    assert!(named(&table, 0, "Secret reader").unwrap().read);
    for seat in 1..seat_count {
        assert!(
            named(&table, seat, "Secret reader").is_none(),
            "seat {seat}"
        );
        assert!(
            table.views[seat]
                .objects
                .iter()
                .all(|o| !o.name.is_empty() || !o.read),
            "a hidden object says nothing of its card"
        );
    }
}

#[test]
fn the_read_member_survives_the_wire_and_is_absent_from_older_messages() {
    let table = table(2);
    let reader = named(&table, 0, "Open reader").unwrap();

    let json = serde_json::to_string(reader).unwrap();
    assert_eq!(&serde_json::from_str::<ObjectView>(&json).unwrap(), reader);

    let mut older: serde_json::Value = serde_json::from_str(&json).unwrap();
    older.as_object_mut().unwrap().remove("read");
    assert!(!serde_json::from_value::<ObjectView>(older).unwrap().read);
}
