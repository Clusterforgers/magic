//! Goal: presence is public: every viewer, a spectator included, reads the same `connected` flag of every seat, a view made from the engine state alone says every seat is connected, and a `Presence` message carries only a seat, a flag and the time before the game answers for an away seat, never an object.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_core::ids::PlayerId;
use mtg_engine::Engine;
use mtg_protocol::{ServerMessage, StateView};
use rstest::rstest;
use serde_json::{Value, json};

use crate::common::unique_setup;

#[rstest]
fn every_viewer_reads_the_same_presence_of_every_seat(
    #[values(2, 3, 4, 6)] seats: usize,
    #[values(1, 9)] seed: u64,
) {
    let engine = Engine::new(unique_setup(seats, seed)).unwrap();
    // Every seat and one spectator.
    let views: Vec<StateView> = (0..=seats)
        .map(|viewer| StateView::for_player(engine.state(), PlayerId::from_index(viewer)))
        .collect();
    for view in &views {
        let flags: Vec<(PlayerId, bool)> = view
            .table
            .players
            .iter()
            .map(|p| (p.seat, p.connected))
            .collect();
        let expected: Vec<(PlayerId, bool)> = (0..seats)
            .map(|s| (PlayerId::from_index(s), true))
            .collect();
        assert_eq!(flags, expected, "viewer {:?}", view.table.viewer);
    }
}

#[rstest]
#[case(0, true, None)]
#[case(1, false, None)]
#[case(5, false, Some(12_000))]
fn a_presence_message_carries_a_seat_a_flag_and_a_time_and_nothing_else(
    #[case] seat: usize,
    #[case] connected: bool,
    #[case] deadline_ms: Option<u64>,
) {
    let message = ServerMessage::Presence {
        seat: PlayerId::from_index(seat),
        connected,
        deadline_ms,
    };
    let value: Value = serde_json::to_value(&message).unwrap();
    let mut expected = json!({"type": "Presence", "seat": seat, "connected": connected});
    if let Some(ms) = deadline_ms {
        expected["deadline_ms"] = json!(ms);
    }
    assert_eq!(value, expected);
}
