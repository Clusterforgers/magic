//! Goal: over a real socket the pace messages have the documented shape: `AutoPass` is a flat tagged message the server accepts, `Welcome` states the seat's setting and a decision states its clock or leaves the field out; a malformed `AutoPass` is a bad message and nothing more; `Yield` and `CancelYield` are flat tagged messages, `Yielding` and `Welcome.yielding` tell the seat what the server holds, and a yield the server cannot read or use is a bad message.
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;
mod net;

use mtg_core::ids::PlayerId;
use mtg_protocol::{ClientMessage, ErrorCode, ServerMessage};
use mtg_server::{GameConfig, Pace, ServerConfig};
use mtg_testkit::Scenario;
use mtg_testkit::cards::instant;
use net::{Frame, TestServer, WsClient, bot_config, connect, create_game, two_seats};
use serde_json::Value;

/// Bot-sized queues and rate limits with the default pace: auto-pass on, clocks running.
fn paced() -> ServerConfig {
    let bots = bot_config();
    ServerConfig {
        fixed_seed: Some(1),
        game: GameConfig {
            pace: Pace::default(),
            ..bots.game
        },
        ..bots
    }
}

async fn welcomed(server: &TestServer, game: &str, token: &str) -> (WsClient, bool, String) {
    let mut socket = connect(server.addr, game, token).await;
    socket.join(game).await;
    let ServerMessage::Welcome { auto_pass, .. } = socket.message().await else {
        panic!("the first message is the welcome");
    };
    let text = socket.texts[0].clone();
    (socket, auto_pass, text)
}

#[tokio::test]
async fn auto_pass_is_a_wire_message_and_welcome_reports_it() {
    let server = TestServer::start(paced()).await;
    let (game, tokens) = create_game(server.addr, two_seats()).await;

    let (mut first, on, text) = welcomed(&server, &game, &tokens[0]).await;
    assert!(on, "the server default is on");
    assert!(text.contains(r#""auto_pass":true"#), "{text}");
    first
        .send_text(r#"{"type":"AutoPass","enabled":false}"#)
        .await;

    // A new socket for the seat takes over and is told what the server holds for it.
    let (mut second, on, text) = welcomed(&server, &game, &tokens[0]).await;
    assert!(!on);
    assert!(text.contains(r#""auto_pass":false"#), "{text}");
    second
        .send(&ClientMessage::AutoPass { enabled: true })
        .await;
    let (_, on, _) = welcomed(&server, &game, &tokens[0]).await;
    assert!(on, "and back on");

    let (_, other, _) = welcomed(&server, &game, &tokens[1]).await;
    assert!(other, "another seat keeps its own");
}

#[tokio::test]
async fn a_malformed_auto_pass_is_a_bad_message_and_changes_nothing() {
    let server = TestServer::start(paced()).await;
    let (game, tokens) = create_game(server.addr, two_seats()).await;
    let (mut socket, _, _) = welcomed(&server, &game, &tokens[0]).await;

    for bad in [
        r#"{"type":"AutoPass"}"#,
        r#"{"type":"AutoPass","enabled":"yes"}"#,
        r#"{"type":"AutoPass","enabled":null}"#,
    ] {
        socket.send_text(bad).await;
        let reply = socket.next().await;
        assert!(
            matches!(
                reply,
                Frame::Message(ServerMessage::Error {
                    code: ErrorCode::BadMessage,
                    ..
                })
            ),
            "{bad} gets a bad-message error: {reply:?}"
        );
    }
    let (_, on, _) = welcomed(&server, &game, &tokens[0]).await;
    assert!(on, "the seat keeps its setting");
}

/// Waits for the next decision on `socket` and returns its frame as JSON.
async fn next_decision(socket: &mut WsClient) -> Value {
    loop {
        let Frame::Message(message) = socket.next().await else {
            panic!("the socket ended before a decision");
        };
        if matches!(message, ServerMessage::Decision { .. }) {
            return serde_json::from_str(socket.texts.last().unwrap()).unwrap();
        }
    }
}

#[tokio::test]
async fn a_decision_without_a_clock_leaves_the_field_out() {
    let server = TestServer::start(paced()).await;
    let (game, tokens) = create_game(server.addr, two_seats()).await;
    let handle = server.registry.handle(game.parse().unwrap()).unwrap();
    let (mut first, _, _) = welcomed(&server, &game, &tokens[0]).await;
    let (mut second, _, _) = welcomed(&server, &game, &tokens[1]).await;
    for seat in 0..2 {
        handle
            .send(PlayerId::from_index(seat), ClientMessage::Ready)
            .await
            .unwrap();
    }
    let waiting = handle.status().await.unwrap().waiting_on.unwrap().index();
    let socket = if waiting == 0 {
        &mut first
    } else {
        &mut second
    };
    let json = next_decision(socket).await;
    assert_eq!(json["type"], "Decision");
    assert!(
        !json.as_object().unwrap().contains_key("deadline_ms"),
        "a mulligan has no clock, so the field is absent: {json}"
    );
}

#[tokio::test]
async fn a_decision_on_a_clock_states_the_milliseconds_it_has() {
    let server = TestServer::start(paced()).await;
    // Seat 1 holds a free instant, so it can respond in every window of seat 0's turn.
    let setup = Scenario::new(2)
        .hand(1, [instant("Zap", 0, &[])])
        .setup()
        .unwrap();
    let created = server.registry.register(setup, Vec::new()).await.unwrap();
    let game = created.id.to_string();
    let tokens: Vec<String> = created
        .seats
        .iter()
        .map(|(_, token)| token.expose().to_owned())
        .collect();
    let handle = server.registry.handle(created.id).unwrap();
    let (mut first, _, _) = welcomed(&server, &game, &tokens[0]).await;
    let (mut second, _, _) = welcomed(&server, &game, &tokens[1]).await;
    for seat in 0..2 {
        handle
            .send(PlayerId::from_index(seat), ClientMessage::Ready)
            .await
            .unwrap();
    }

    let json = next_decision(&mut second).await;
    assert_eq!(json["deadline_ms"], 10_000, "{json}");
    // The same decision after a reconnect carries what is left of it.
    let (mut again, _, _) = welcomed(&server, &game, &tokens[1]).await;
    let json = next_decision(&mut again).await;
    let left = json["deadline_ms"].as_u64().unwrap();
    assert!((1..=10_000).contains(&left), "{json}");
    // Seat 0 had nothing to do and was passed for, so it was never asked anything.
    let mut asked = false;
    while let Frame::Message(message) = first.next().await {
        asked |= matches!(message, ServerMessage::Decision { .. });
        if matches!(message, ServerMessage::Events(_)) {
            break;
        }
    }
    assert!(!asked);
}

/// Waits for the next `Yielding` frame on `socket` and returns it as JSON.
async fn next_yielding(socket: &mut WsClient) -> Value {
    loop {
        let Frame::Message(message) = socket.next().await else {
            panic!("the socket ended before a Yielding");
        };
        if matches!(message, ServerMessage::Yielding { .. }) {
            return serde_json::from_str(socket.texts.last().unwrap()).unwrap();
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

#[tokio::test]
async fn yields_are_flat_wire_messages_told_back_in_a_notice_and_in_the_welcome() {
    let server = TestServer::start(paced()).await;
    let (game, tokens) = create_game(server.addr, two_seats()).await;
    let handle = server.registry.handle(game.parse().unwrap()).unwrap();
    let (mut first, _, text) = welcomed(&server, &game, &tokens[0]).await;
    assert!(text.contains(r#""yielding":null"#), "{text}");

    // Before the game starts there is nothing to pass for; cancelling nothing is no news.
    first
        .send_text(r#"{"type":"Yield","until":"EndOfTurn"}"#)
        .await;
    assert_eq!(next_error(&mut first).await, ErrorCode::BadMessage);
    first.send_text(r#"{"type":"CancelYield"}"#).await;

    let (_, _, other) = welcomed(&server, &game, &tokens[1]).await;
    assert!(other.contains(r#""yielding":null"#), "{other}");
    for seat in 0..2 {
        handle
            .send(PlayerId::from_index(seat), ClientMessage::Ready)
            .await
            .unwrap();
    }

    // Malformed asks are bad messages and change nothing.
    for bad in [
        r#"{"type":"Yield"}"#,
        r#"{"type":"Yield","until":"Never"}"#,
        r#"{"type":"Yield","until":null}"#,
        r#"{"type":"Yield","until":3}"#,
    ] {
        first.send_text(bad).await;
        assert_eq!(next_error(&mut first).await, ErrorCode::BadMessage, "{bad}");
    }

    // A good one is told back as one flat message, and so is its end.
    first
        .send_text(r#"{"type":"Yield","until":"MyTurn"}"#)
        .await;
    assert_eq!(
        next_yielding(&mut first).await,
        serde_json::json!({"type": "Yielding", "until": "MyTurn"})
    );
    first
        .send(&ClientMessage::Yield {
            until: mtg_protocol::YieldUntil::Resolve,
        })
        .await;
    assert_eq!(
        next_yielding(&mut first).await,
        serde_json::json!({"type": "Yielding", "until": "Resolve"}),
        "a new yield replaces the old one"
    );

    // A socket that joins later is told what the server holds, and another seat is not told of it.
    let (mut again, _, text) = welcomed(&server, &game, &tokens[0]).await;
    assert!(text.contains(r#""yielding":"Resolve""#), "{text}");
    let (_, _, text) = welcomed(&server, &game, &tokens[1]).await;
    assert!(text.contains(r#""yielding":null"#), "{text}");

    again.send_text(r#"{"type":"CancelYield"}"#).await;
    assert_eq!(
        next_yielding(&mut again).await,
        serde_json::json!({"type": "Yielding", "until": null})
    );
    let (_, _, text) = welcomed(&server, &game, &tokens[0]).await;
    assert!(text.contains(r#""yielding":null"#), "{text}");
}
