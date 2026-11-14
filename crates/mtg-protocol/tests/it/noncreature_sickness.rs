//! Goal: only a creature is shown summoning sick (CR 302.6): a land or other noncreature permanent that arrived this turn is never marked held back in any seat's view, while a creature that arrived with it is.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_engine::turn::Step;
use mtg_testkit::cards::{forest, free_creature, island, mountain, plains, swamp, wastes};
use mtg_testkit::{Scenario, sick};
use rstest::rstest;

use crate::common::table::Table;

/// Whether the seat's view of the permanent named `name` says it is held back.
fn viewed(table: &Table, viewer: usize, name: &str) -> bool {
    let id = table.driver.id_of(name).unwrap();
    table.views[viewer]
        .objects
        .iter()
        .find(|object| object.id == id)
        .and_then(|object| object.permanent.as_ref())
        .map(|permanent| permanent.summoning_sick)
        .unwrap()
}

#[rstest]
#[case::forest(forest())]
#[case::island(island())]
#[case::plains(plains())]
#[case::swamp(swamp())]
#[case::mountain(mountain())]
#[case::wastes(wastes())]
fn a_land_that_arrived_this_turn_is_not_shown_sick(
    #[case] land: Arc<CardDefinition>,
    #[values(2, 4)] seats: usize,
) {
    let name = land.name.clone();
    let table = Table::new(
        Scenario::new(seats)
            .at(1, 0, Step::Main1)
            .battlefield(0, [sick(land), sick(free_creature("Arrived", 2, 2))]),
    );
    for viewer in 0..seats {
        assert!(
            !viewed(&table, viewer, &name),
            "seat {viewer}'s view of {name}"
        );
        assert!(
            viewed(&table, viewer, "Arrived"),
            "seat {viewer}'s view of the creature"
        );
    }
}
