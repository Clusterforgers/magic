//! Goal: everything that is not a well-formed, authenticated join (wrong token, unknown game, bad origin, version mismatch, a first message that is not Join) is refused before it can touch the game, and a client can never choose its own seat.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::net;

use mtg_core::ids::PlayerId;
use mtg_engine::state::GameResult;
use mtg_protocol::{ClientMessage, ErrorCode, PROTOCOL_VERSION, ServerMessage};
use mtg_server::ServerConfig;
use net::{Table, table_config, two_seats};
use rstest::rstest;
use serde_json::json;

#[tokio::test]
async fn unknown_games_and_wrong_tokens_get_one_and_the_same_refusal() {
    let table = Table::start(table_config(), true).await;
    let (other_game, other_tokens) = net::create_game(table.addr(), two_seats()).await;
    let real = table.tokens[0].as_str();
    let wrong = "0123456789abcdef".repeat(4);
    let long = "a".repeat(5_000);
    let upper = real.to_ascii_uppercase();
    let missing = "11111111-2222-4333-8444-555555555555";
    let probes = [
        ("an unknown game", missing, real),
        ("a wrong token", table.game.as_str(), wrong.as_str()),
        (
            "a token of another game",
            table.game.as_str(),
            other_tokens[0].as_str(),
        ),
        ("the token on another game", other_game.as_str(), real),
        ("a short token", table.game.as_str(), "short"),
        ("a truncated token", table.game.as_str(), &real[..63]),
        ("an upper-case token", table.game.as_str(), upper.as_str()),
        (
            "an encoded token",
            table.game.as_str(),
            "%C3%BCn%C3%AFc%C3%B6d%C3%A9",
        ),
        ("a broken escape", table.game.as_str(), "%ZZ%ZZ"),
        ("a huge token", table.game.as_str(), long.as_str()),
        ("a game that is no uuid", "not-a-uuid", real),
        ("the nil uuid", "00000000-0000-0000-0000-000000000000", real),
    ];
    let mut bodies = Vec::new();
    for (what, game, token) in probes {
        let (status, body) = net::refused(table.addr(), game, token, None).await;
        assert_eq!(status, 404, "{what}");
        bodies.push((what, body));
    }
    let first = bodies[0].1.clone();
    for (what, body) in &bodies {
        assert_eq!(*body, first, "{what} is told apart from an unknown game");
    }
    assert!(first.contains("not_found"));
    table.nobody_connected().await;
    table.assert_untouched().await;
}

#[tokio::test]
async fn only_allowed_origins_and_the_servers_own_host_may_open_a_socket() {
    let config = ServerConfig {
        allowed_origins: vec!["http://localhost:5173".into()],
        ..table_config()
    };
    let table = Table::start(config, false).await;
    let (game, token) = (table.game.as_str(), table.tokens[0].as_str());
    let own = format!("http://{}", table.addr());
    for origin in [
        "https://evil.example",
        "http://localhost:5174",
        "null",
        "http://127.0.0.1",
    ] {
        let (status, body) = net::refused(table.addr(), game, token, Some(origin)).await;
        assert_eq!(status, 403, "{origin}");
        assert!(body.contains("forbidden_origin"));
    }
    table.nobody_connected().await;

    for origin in [Some("http://localhost:5173"), Some(own.as_str()), None] {
        let mut client = net::try_connect(table.addr(), game, token, origin)
            .await
            .unwrap_or_else(|e| panic!("{origin:?} was refused: {e}"));
        assert_eq!(client.welcome(game).await.index(), 0);
        client.close().await;
    }
}

#[tokio::test]
async fn a_plain_get_is_not_a_socket() {
    let table = Table::start(table_config(), false).await;
    let path = format!("/ws/{}/{}", table.game, table.tokens[0]);
    let reply = net::get(table.addr(), &path).await;
    assert!((400..500).contains(&reply.status), "{}", reply.status);
}

/// What a client sends first instead of a proper join.
#[derive(Debug, Clone, Copy)]
enum First {
    Concede,
    Ready,
    Version(u32),
    OtherGame,
    NotAGameId,
    Garbage,
    Empty,
    Binary,
    MissingFields,
    InAnArray,
}

fn join_text(version: u32, game: &str) -> String {
    json!({ "type": "Join", "protocol_version": version, "game": game, "name": "x" }).to_string()
}

#[rstest]
#[case(First::Concede, ErrorCode::BadMessage)]
#[case(First::Ready, ErrorCode::BadMessage)]
#[case(First::Version(PROTOCOL_VERSION + 1), ErrorCode::UnsupportedVersion)]
#[case(First::Version(0), ErrorCode::UnsupportedVersion)]
#[case(First::Version(u32::MAX), ErrorCode::UnsupportedVersion)]
#[case(First::OtherGame, ErrorCode::UnknownGame)]
#[case(First::NotAGameId, ErrorCode::UnknownGame)]
#[case(First::Garbage, ErrorCode::BadMessage)]
#[case(First::Empty, ErrorCode::BadMessage)]
#[case(First::Binary, ErrorCode::BadMessage)]
#[case(First::MissingFields, ErrorCode::BadMessage)]
#[case(First::InAnArray, ErrorCode::BadMessage)]
#[tokio::test]
async fn a_first_message_that_is_not_a_good_join_gets_an_error_and_a_close(
    #[case] first: First,
    #[case] expected: ErrorCode,
) {
    let table = Table::start(table_config(), true).await;
    let mut client = net::connect(table.addr(), &table.game, &table.tokens[0]).await;
    let game = table.game.as_str();
    let stranger = "11111111-2222-4333-8444-555555555555";
    match first {
        First::Concede => client.send(&ClientMessage::Concede).await,
        First::Ready => client.send(&ClientMessage::Ready).await,
        First::Version(v) => client.send_text(join_text(v, game)).await,
        First::OtherGame => {
            client
                .send_text(join_text(PROTOCOL_VERSION, stranger))
                .await;
        }
        First::NotAGameId => client.send_text(join_text(PROTOCOL_VERSION, "nope")).await,
        First::Garbage => client.send_text("this is not json").await,
        First::Empty => client.send_text("").await,
        First::Binary => client.send_binary(vec![0, 159, 146, 150]).await,
        First::MissingFields => client.send_text(r#"{"type":"Join"}"#).await,
        First::InAnArray => {
            let text = format!("[{}]", join_text(PROTOCOL_VERSION, game));
            client.send_text(text).await;
        }
    }
    match client.message().await {
        ServerMessage::Error { code, .. } => assert_eq!(code, expected),
        other => panic!("expected an error, got {other:?}"),
    }
    assert_eq!(client.closed().await, Some(1002));
    table.nobody_connected().await;
    table.assert_untouched().await;
}

#[tokio::test]
async fn a_message_cannot_name_a_seat_other_than_the_tokens() {
    let table = Table::start(table_config(), true).await;
    let mut client = table.player(0).await;
    client
        .send_text(r#"{"type":"Concede","seat":1,"player":1,"from":1}"#)
        .await;
    // Seat 0 conceded, so seat 1 won.
    let result = loop {
        if let ServerMessage::GameOver { result } = client.message().await {
            break result;
        }
    };
    assert_eq!(result, GameResult::Winner(PlayerId::from_index(1)));
}
