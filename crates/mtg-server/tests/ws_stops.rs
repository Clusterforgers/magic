//! Goal: over a real socket stops and Full Control are flat tagged messages the server accepts as written, `Stops`, `FullControl` and `Welcome` tell the seat what the server holds in the documented shape, and a step where nobody gets priority or a message the server cannot read is a bad message that leaves the socket open and the stops as they were.
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;
mod net;

use mtg_protocol::{ClientMessage, ErrorCode, FullControlMode, ServerMessage};
use mtg_server::ServerConfig;
use net::{Frame, TestServer, WsClient, bot_config, connect, create_game, two_seats};
use serde_json::{Value, json};

/// Joins on a fresh socket and returns it with the verbatim text of the `Welcome`.
async fn welcomed(server: &TestServer, game: &str, token: &str) -> (WsClient, Value) {
    let mut socket = connect(server.addr, game, token).await;
    socket.join(game).await;
    let ServerMessage::Welcome { .. } = socket.message().await else {
        panic!("the first message is the welcome");
    };
    let text: Value = serde_json::from_str(&socket.texts[0]).unwrap();
    (socket, text)
}

/// Waits for the next frame of the given `type` and returns it as JSON.
async fn next_of(socket: &mut WsClient, kind: &str) -> Value {
    loop {
        let Frame::Message(_) = socket.next().await else {
            panic!("the socket ended before a {kind}");
        };
        let frame: Value = serde_json::from_str(socket.texts.last().unwrap()).unwrap();
        if frame["type"] == kind {
            return frame;
        }
    }
}

/// Waits for the next `Error` frame on `socket` and returns its code.
async fn next_error(socket: &mut WsClient) -> ErrorCode {
    loop {
        let Frame::Message(message) = socket.next().await else {
            panic!("the socket ended before an Error");
        };
        if let ServerMessage::Error { code, .. } = message {
            return code;
        }
    }
}

fn config() -> ServerConfig {
    bot_config()
}

#[tokio::test]
async fn stops_and_full_control_are_flat_messages_told_back_whole_and_in_the_welcome() {
    let server = TestServer::start(config()).await;
    let (game, tokens) = create_game(server.addr, two_seats()).await;
    let (mut first, welcome) = welcomed(&server, &game, &tokens[0]).await;
    assert_eq!(welcome["stops"], json!([]), "{welcome}");
    assert_eq!(welcome["full_control"], "Off", "{welcome}");

    // Stops are held from the lobby on; each change is told back as the whole list, own side first, each stop
    // with the `keep` it was set with.
    first
        .send_text(r#"{"type":"SetStop","side":"Opponent","step":"End","on":true,"keep":true}"#)
        .await;
    assert_eq!(
        next_of(&mut first, "Stops").await,
        json!({"type": "Stops", "stops": [{"side": "Opponent", "step": "End", "keep": true}]})
    );
    first
        .send_text(r#"{"type":"SetStop","side":"Own","step":"Main1","on":true,"keep":false}"#)
        .await;
    assert_eq!(
        next_of(&mut first, "Stops").await,
        json!({"type": "Stops", "stops": [
            {"side": "Own", "step": "Main1", "keep": false},
            {"side": "Opponent", "step": "End", "keep": true},
        ]})
    );
    // The same stop again with the other `keep` is a change, told as the list in the same order; the same
    // request once more is not.
    first
        .send_text(r#"{"type":"SetStop","side":"Own","step":"Main1","on":true,"keep":true}"#)
        .await;
    assert_eq!(
        next_of(&mut first, "Stops").await,
        json!({"type": "Stops", "stops": [
            {"side": "Own", "step": "Main1", "keep": true},
            {"side": "Opponent", "step": "End", "keep": true},
        ]})
    );
    first
        .send_text(r#"{"type":"SetStop","side":"Own","step":"Main1","on":true,"keep":true}"#)
        .await;
    first
        .send_text(r#"{"type":"SetStop","side":"Own","step":"Main1","on":true,"keep":false}"#)
        .await;
    assert_eq!(
        next_of(&mut first, "Stops").await,
        json!({"type": "Stops", "stops": [
            {"side": "Own", "step": "Main1", "keep": false},
            {"side": "Opponent", "step": "End", "keep": true},
        ]}),
        "the repeat said nothing, so the next word is the downgrade"
    );
    first
        .send_text(r#"{"type":"FullControl","mode":"Held"}"#)
        .await;
    assert_eq!(
        next_of(&mut first, "FullControl").await,
        json!({"type": "FullControl", "mode": "Held"})
    );
    first
        .send(&ClientMessage::FullControl {
            mode: FullControlMode::NextWindow,
        })
        .await;
    assert_eq!(
        next_of(&mut first, "FullControl").await,
        json!({"type": "FullControl", "mode": "NextWindow"}),
        "a new mode replaces the old one"
    );

    // A socket that joins later is told what the server holds, and another seat is told of none of it.
    let (mut again, welcome) = welcomed(&server, &game, &tokens[0]).await;
    assert_eq!(
        welcome["stops"],
        json!([
            {"side": "Own", "step": "Main1", "keep": false},
            {"side": "Opponent", "step": "End", "keep": true},
        ])
    );
    assert_eq!(welcome["full_control"], "NextWindow");
    let (_, other) = welcomed(&server, &game, &tokens[1]).await;
    assert_eq!(other["stops"], json!([]));
    assert_eq!(other["full_control"], "Off");

    // Clearing names the stop by side and step: the `keep` it carries need not be the one the stop has.
    again
        .send_text(r#"{"type":"SetStop","side":"Own","step":"Main1","on":false,"keep":true}"#)
        .await;
    assert_eq!(
        next_of(&mut again, "Stops").await,
        json!({"type": "Stops", "stops": [{"side": "Opponent", "step": "End", "keep": true}]})
    );
}

#[tokio::test]
async fn a_step_where_nobody_gets_priority_is_a_bad_message_and_the_socket_and_the_stops_stay() {
    let server = TestServer::start(config()).await;
    let (game, tokens) = create_game(server.addr, two_seats()).await;
    let (mut socket, _) = welcomed(&server, &game, &tokens[0]).await;
    for side in ["Own", "Opponent"] {
        for step in ["Untap", "Cleanup"] {
            for on in [true, false] {
                let text = format!(
                    r#"{{"type":"SetStop","side":"{side}","step":"{step}","on":{on},"keep":false}}"#
                );
                socket.send_text(text.clone()).await;
                assert_eq!(
                    next_error(&mut socket).await,
                    ErrorCode::BadMessage,
                    "{text}"
                );
            }
        }
    }
    // The connection is still open and a valid stop still works on it.
    socket
        .send_text(r#"{"type":"SetStop","side":"Own","step":"Upkeep","on":true,"keep":false}"#)
        .await;
    assert_eq!(
        next_of(&mut socket, "Stops").await,
        json!({"type": "Stops", "stops": [{"side": "Own", "step": "Upkeep", "keep": false}]}),
        "only the valid one was kept"
    );
}

#[tokio::test]
async fn malformed_stops_and_modes_are_bad_messages_and_change_nothing() {
    let server = TestServer::start(config()).await;
    let (game, tokens) = create_game(server.addr, two_seats()).await;
    let (mut socket, _) = welcomed(&server, &game, &tokens[0]).await;
    for bad in [
        r#"{"type":"SetStop"}"#,
        r#"{"type":"SetStop","side":"Own","step":"Main1","on":true}"#,
        r#"{"type":"SetStop","side":"own","step":"Main1","on":true,"keep":false}"#,
        r#"{"type":"SetStop","side":"Own","step":"Main 1","on":true,"keep":false}"#,
        r#"{"type":"SetStop","side":"Own","step":"Main1","on":"yes","keep":false}"#,
        r#"{"type":"SetStop","side":"Own","step":3,"on":true,"keep":false}"#,
        r#"{"type":"FullControl"}"#,
        r#"{"type":"FullControl","mode":"Always"}"#,
        r#"{"type":"FullControl","mode":"held"}"#,
        r#"{"type":"FullControl","mode":true}"#,
        r#"{"type":"FullControl","mode":null}"#,
    ] {
        socket.send_text(bad).await;
        assert_eq!(
            next_error(&mut socket).await,
            ErrorCode::BadMessage,
            "{bad}"
        );
        // A message the server reads, however little it does, restarts its count of bad ones in a row.
        socket.send_text(r#"{"type":"CancelYield"}"#).await;
    }
    let (_, welcome) = welcomed(&server, &game, &tokens[0]).await;
    assert_eq!(welcome["stops"], json!([]), "{welcome}");
    assert_eq!(welcome["full_control"], "Off", "{welcome}");
}
