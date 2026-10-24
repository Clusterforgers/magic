//! Goal: the turn view tells every seat at two to four seats whether the turn is an extra turn and which extra turns wait (left out of the JSON while there are none), the events that add, skip and begin extra turns and the one that adds phases reach every seat whole, and a table view with extra turns waiting round-trips through its JSON.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_core::ids::PlayerId;
use mtg_engine::turn::Step;
use mtg_engine::{Event, apply_event};
use mtg_protocol::{EventView, TableView, project_events};
use mtg_testkit::{Driver, Scenario};
use rstest::rstest;

fn seat(index: usize) -> PlayerId {
    PlayerId::from_index(index)
}

#[rstest]
fn the_turn_view_names_the_extra_turns_to_every_seat(#[values(2, 3, 4)] seats: usize) {
    let driver = Driver::from_scenario(Scenario::new(seats)).unwrap();
    let mut state = driver.state().clone();
    for viewer in 0..seats {
        let view = TableView::of(&state, seat(viewer));
        assert!(!view.turn.extra);
        assert_eq!(view.turn.extra_turns, Vec::<PlayerId>::new());
        let json = serde_json::to_value(&view).unwrap();
        assert!(json["turn"].get("extra").is_none(), "left out while false");
        assert!(
            json["turn"].get("extra_turns").is_none(),
            "left out while none wait"
        );
    }
    let before = state.clone();
    let added = [
        Event::ExtraTurnAdded {
            player: seat(0),
            skip_untap: false,
        },
        Event::ExtraTurnAdded {
            player: seat(1),
            skip_untap: true,
        },
        Event::PhasesAdded {
            steps: vec![Step::BeginCombat, Step::Main2],
        },
    ];
    for event in &added {
        apply_event(&mut state, event).unwrap();
    }
    for viewer in 0..seats {
        let view = TableView::of(&state, seat(viewer));
        assert_eq!(view.turn.extra_turns, [seat(0), seat(1)]);
        let json = serde_json::to_value(&view).unwrap();
        assert_eq!(json["turn"]["extra_turns"], serde_json::json!([0, 1]));
        let back: TableView = serde_json::from_value(json).unwrap();
        assert_eq!(back, view);
    }
    let viewers: Vec<PlayerId> = (0..seats).map(seat).collect();
    let batches = project_events(&before, &added, &viewers, 0).unwrap();
    for batch in &batches {
        for event in &added {
            assert!(
                batch
                    .events
                    .iter()
                    .any(|told| matches!(told, EventView::Public(seen) if seen == event)),
                "every seat is told {event:?}"
            );
        }
    }
}

#[rstest]
fn a_turn_that_is_extra_says_so_to_every_seat(#[values(2, 4)] seats: usize) {
    let driver = Driver::from_scenario(Scenario::new(seats)).unwrap();
    let mut state = driver.state().clone();
    let number = state.turn().number() + 1;
    let events = [
        Event::ExtraTurnAdded {
            player: seat(0),
            skip_untap: false,
        },
        Event::TurnBegan {
            number,
            active: seat(0),
            extra: true,
        },
    ];
    let before = state.clone();
    for event in &events {
        apply_event(&mut state, event).unwrap();
    }
    for viewer in 0..seats {
        let view = TableView::of(&state, seat(viewer));
        assert!(view.turn.extra);
        assert_eq!(
            view.turn.extra_turns,
            Vec::<PlayerId>::new(),
            "the turn was taken"
        );
        let json = serde_json::to_value(&view).unwrap();
        assert_eq!(json["turn"]["extra"], true);
    }
    let viewers: Vec<PlayerId> = (0..seats).map(seat).collect();
    let batches = project_events(&before, &events, &viewers, 0).unwrap();
    for batch in &batches {
        assert!(batch.events.iter().any(|told| matches!(
            told,
            EventView::Public(Event::TurnBegan { extra: true, .. })
        )));
    }
}
