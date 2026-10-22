//! Goal: the turn view names the monarch to every seat at two to four seats and leaves the field out while nobody is, the event that makes a player the monarch reaches every seat whole, and a table view with a monarch round-trips through its JSON.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_core::ids::PlayerId;
use mtg_engine::{Event, apply_event};
use mtg_protocol::{EventView, TableView, project_events};
use mtg_testkit::{Driver, Scenario};
use rstest::rstest;

fn seat(index: usize) -> PlayerId {
    PlayerId::from_index(index)
}

#[rstest]
fn the_turn_view_names_the_monarch_to_every_seat(#[values(2, 3, 4)] seats: usize) {
    let driver = Driver::from_scenario(Scenario::new(seats)).unwrap();
    let mut state = driver.state().clone();
    for viewer in 0..seats {
        let view = TableView::of(&state, seat(viewer));
        assert_eq!(view.turn.monarch, None);
        let json = serde_json::to_value(&view).unwrap();
        assert!(
            json["turn"].get("monarch").is_none(),
            "left out while nobody is"
        );
    }
    let before = state.clone();
    let event = Event::MonarchChanged { player: seat(1) };
    apply_event(&mut state, &event).unwrap();
    for viewer in 0..seats {
        let view = TableView::of(&state, seat(viewer));
        assert_eq!(view.turn.monarch, Some(seat(1)));
        let json = serde_json::to_value(&view).unwrap();
        assert_eq!(json["turn"]["monarch"], 1);
        let back: TableView = serde_json::from_value(json).unwrap();
        assert_eq!(back, view);
    }
    let viewers: Vec<PlayerId> = (0..seats).map(seat).collect();
    let batches = project_events(&before, std::slice::from_ref(&event), &viewers, 0).unwrap();
    for batch in &batches {
        assert!(
            batch
                .events
                .iter()
                .any(|told| matches!(told, EventView::Public(seen) if *seen == event)),
            "every seat is told who became the monarch"
        );
    }
}
