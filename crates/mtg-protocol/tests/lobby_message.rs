//! Goal: the lobby roster and `NotReady` keep their documented wire shape, survive a JSON round trip for any table of 2 to 12 seats in any mix of states, refuse malformed bodies, and reach the TypeScript bindings.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeMap;

use mtg_core::ids::PlayerId;
use mtg_protocol::bindings::export_to;
use mtg_protocol::{ClientMessage, LobbySeat, ServerMessage};
use proptest::prelude::*;
use rstest::rstest;
use serde_json::{Value, json};

fn seat(index: u64, name: &str, connected: bool, ready: bool) -> LobbySeat {
    LobbySeat {
        seat: PlayerId(index),
        name: name.to_owned(),
        connected,
        ready,
        deck: None,
    }
}

#[test]
fn the_roster_has_the_documented_wire_shape() {
    let message = ServerMessage::Lobby {
        seats: vec![
            seat(0, "Ana", true, true),
            seat(1, "Bo", true, false),
            seat(2, "Cy", false, false),
        ],
        needed: 2,
    };
    let value = serde_json::to_value(&message).unwrap();
    assert_eq!(
        value,
        json!({
            "type": "Lobby",
            "seats": [
                {"seat": 0, "name": "Ana", "connected": true, "ready": true},
                {"seat": 1, "name": "Bo", "connected": true, "ready": false},
                {"seat": 2, "name": "Cy", "connected": false, "ready": false},
            ],
            "needed": 2,
        })
    );
    assert_eq!(
        serde_json::from_value::<ServerMessage>(value).unwrap(),
        message
    );
}

#[test]
fn not_ready_is_a_bare_tagged_object() {
    let value = serde_json::to_value(ClientMessage::NotReady).unwrap();
    assert_eq!(value, json!({"type": "NotReady"}));
    assert_eq!(
        serde_json::from_value::<ClientMessage>(value).unwrap(),
        ClientMessage::NotReady
    );
    // It is its own message, not a spelling of `Ready`.
    assert_ne!(
        serde_json::to_value(ClientMessage::Ready).unwrap(),
        serde_json::to_value(ClientMessage::NotReady).unwrap()
    );
}

#[rstest]
#[case::no_seats(json!({"type": "Lobby", "needed": 0}))]
#[case::no_needed(json!({"type": "Lobby", "seats": []}))]
#[case::needed_is_text(json!({"type": "Lobby", "seats": [], "needed": "2"}))]
#[case::needed_is_negative(json!({"type": "Lobby", "seats": [], "needed": -1}))]
#[case::needed_is_fractional(json!({"type": "Lobby", "seats": [], "needed": 1.5}))]
#[case::seats_is_an_object(json!({"type": "Lobby", "seats": {}, "needed": 0}))]
#[case::seat_without_ready(json!({
    "type": "Lobby", "needed": 1,
    "seats": [{"seat": 0, "name": "Ana", "connected": true}]
}))]
#[case::seat_without_connected(json!({
    "type": "Lobby", "needed": 1,
    "seats": [{"seat": 0, "name": "Ana", "ready": true}]
}))]
#[case::seat_without_name(json!({
    "type": "Lobby", "needed": 1,
    "seats": [{"seat": 0, "connected": true, "ready": true}]
}))]
#[case::ready_is_a_string(json!({
    "type": "Lobby", "needed": 1,
    "seats": [{"seat": 0, "name": "Ana", "connected": true, "ready": "yes"}]
}))]
#[case::ready_is_null(json!({
    "type": "Lobby", "needed": 1,
    "seats": [{"seat": 0, "name": "Ana", "connected": true, "ready": null}]
}))]
#[case::seat_is_text(json!({
    "type": "Lobby", "needed": 1,
    "seats": [{"seat": "0", "name": "Ana", "connected": true, "ready": true}]
}))]
#[case::seat_is_negative(json!({
    "type": "Lobby", "needed": 1,
    "seats": [{"seat": -1, "name": "Ana", "connected": true, "ready": true}]
}))]
fn malformed_rosters_are_refused(#[case] body: Value) {
    assert!(serde_json::from_value::<ServerMessage>(body).is_err());
}

#[rstest]
#[case::lower_case("notready")]
#[case::snake_case("not_ready")]
#[case::spaced("Not Ready")]
#[case::lobby_from_a_client("Lobby")]
fn other_spellings_of_the_new_messages_are_not_client_messages(#[case] tag: &str) {
    assert!(serde_json::from_value::<ClientMessage>(json!({"type": tag})).is_err());
}

#[test]
fn a_roster_is_not_a_client_message_and_not_ready_is_not_a_server_message() {
    let roster = json!({"type": "Lobby", "seats": [], "needed": 0});
    assert!(serde_json::from_value::<ClientMessage>(roster).is_err());
    assert!(serde_json::from_value::<ServerMessage>(json!({"type": "NotReady"})).is_err());
}

fn name() -> impl Strategy<Value = String> {
    // Real names are 1 to 32 characters; the wire carries whatever the server stored, so include awkward ones.
    prop_oneof![
        "[A-Za-z0-9_ -]{1,32}",
        "\\PC{1,32}",
        Just("Ünïcode \"quoted\" \\ back".to_owned()),
    ]
}

fn table() -> impl Strategy<Value = Vec<LobbySeat>> {
    (2usize..=12).prop_flat_map(|seats| {
        prop::collection::vec((name(), any::<bool>(), any::<bool>()), seats).prop_map(|rows| {
            rows.into_iter()
                .enumerate()
                .map(|(index, (name, connected, ready))| {
                    seat(u64::try_from(index).unwrap(), &name, connected, ready)
                })
                .collect()
        })
    })
}

proptest! {
    #[test]
    fn any_table_round_trips_in_seat_order(seats in table()) {
        let needed = u32::try_from(seats.iter().filter(|s| !s.ready).count()).unwrap();
        let message = ServerMessage::Lobby { seats, needed };
        let text = serde_json::to_string(&message).unwrap();
        let back: ServerMessage = serde_json::from_str(&text).unwrap();
        prop_assert_eq!(&back, &message);
        // One flat object: the tag, the list and the count, nothing else.
        let value: Value = serde_json::from_str(&text).unwrap();
        let mut keys: Vec<&str> = value.as_object().unwrap().keys().map(String::as_str).collect();
        keys.sort_unstable();
        prop_assert_eq!(keys, ["needed", "seats", "type"]);
        for (index, row) in value["seats"].as_array().unwrap().iter().enumerate() {
            prop_assert_eq!(row["seat"].as_u64().unwrap(), index as u64);
            let mut fields: Vec<&str> = row.as_object().unwrap().keys().map(String::as_str).collect();
            fields.sort_unstable();
            prop_assert_eq!(fields, ["connected", "name", "ready", "seat"]);
        }
    }
}

fn generated() -> BTreeMap<String, String> {
    let scratch = tempfile::tempdir().unwrap();
    export_to(scratch.path()).unwrap();
    std::fs::read_dir(scratch.path())
        .unwrap()
        .map(|entry| {
            let path = entry.unwrap().path();
            (
                path.file_name().unwrap().to_string_lossy().into_owned(),
                std::fs::read_to_string(&path).unwrap(),
            )
        })
        .collect()
}

#[test]
fn the_bindings_carry_the_roster_and_not_ready() {
    let files = generated();
    assert!(files.contains_key("LobbySeat.ts"), "missing LobbySeat.ts");
    assert!(files["index.ts"].contains("LobbySeat"));
    for field in [
        "seat: PlayerId",
        "name: string",
        "connected: boolean",
        "ready: boolean",
    ] {
        assert!(
            files["LobbySeat.ts"].contains(field),
            "LobbySeat lacks {field}"
        );
    }
    let server = &files["ServerMessage.ts"];
    assert!(server.contains("\"type\": \"Lobby\""));
    assert!(server.contains("seats: Array<LobbySeat>"));
    assert!(server.contains("needed: number"));
    assert!(files["ClientMessage.ts"].contains("{ \"type\": \"NotReady\" }"));
}
