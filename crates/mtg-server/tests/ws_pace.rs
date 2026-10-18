//! Goal: over a real socket the pace messages have the documented shape: `AutoPass` is a flat tagged message the server accepts, `Welcome` states the seat's setting; a malformed `AutoPass` is a bad message and nothing more.
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;
mod net;

use mtg_protocol::{ClientMessage, ErrorCode, ServerMessage};
use mtg_server::{GameConfig, Pace, ServerConfig};
use net::{Frame, TestServer, WsClient, bot_config, connect, create_game, two_seats};

/// Bot-sized queues and rate limits with the default pace: auto-pass on.
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
