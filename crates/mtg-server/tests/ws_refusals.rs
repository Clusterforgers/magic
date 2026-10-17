//! Goal: everything that is not a well-formed, authenticated join (wrong token, unknown game, bad origin, version mismatch, a first message that is not Join, oversize frames, garbage JSON, floods, silence) is refused or closed without ever touching the game, and a client can never choose its own seat.
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;
mod net;

use std::net::SocketAddr;
use std::time::Duration;

use mtg_core::ids::PlayerId;
use mtg_engine::state::GameResult;
use mtg_protocol::{ClientMessage, ErrorCode, PROTOCOL_VERSION, ServerMessage};
use mtg_server::config::SocketLimits;
use mtg_server::{GameHandle, GameStatus, ServerConfig};
use net::{TestServer, WsClient, bot_config};
use rstest::rstest;
use serde_json::json;

fn two_seats() -> serde_json::Value {
    json!({ "format": "normal", "seats": [
        { "name": "Ann", "deck": "mono-green" },
        { "name": "Bo", "deck": "mono-red" },
    ]})
}

/// The default test config with a fixed seed, so the same seat always moves first.
fn config() -> ServerConfig {
    ServerConfig {
        fixed_seed: Some(1),
        ..bot_config()
    }
}

/// A server with one two-seat game, started (waiting for its first answer) or still in the lobby.
struct Table {
    server: TestServer,
    game: String,
    tokens: Vec<String>,
    handle: GameHandle,
    before: GameStatus,
    /// The seat the running game waits for.
    waiting: usize,
}

impl Table {
    async fn start(config: ServerConfig, started: bool) -> Table {
        let server = TestServer::start(config).await;
        let (game, tokens) = net::create_game(server.addr, two_seats()).await;
        let handle = server.registry.handle(game.parse().unwrap()).unwrap();
        if started {
            for seat in [0, 1] {
                let ready = ClientMessage::Ready;
                handle
                    .send(PlayerId::from_index(seat), ready)
                    .await
                    .unwrap();
            }
        }
        let before = handle.status().await.unwrap();
        assert_eq!(before.started, started);
        Table {
            waiting: before.waiting_on.map_or(0, PlayerId::index),
            server,
            game,
            tokens,
            handle,
            before,
        }
    }

    fn addr(&self) -> SocketAddr {
        self.server.addr
    }

    /// Opens a seat's socket and joins; of what a running game then sends, only `Welcome` is consumed.
    async fn player(&self, seat: usize) -> WsClient {
        let mut client = net::connect(self.addr(), &self.game, &self.tokens[seat]).await;
        let welcomed = client.welcome(&self.game).await;
        assert_eq!(welcomed.index(), seat);
        client
    }

    /// Reads the snapshot and decision a running game sends the seat it waits for.
    async fn expect_resync(&self, client: &mut WsClient) {
        assert!(matches!(
            client.message().await,
            ServerMessage::Snapshot { .. }
        ));
        assert!(matches!(
            client.message().await,
            ServerMessage::Decision { .. }
        ));
    }

    /// The game must be exactly as it was: nothing a refused client sent got through.
    async fn assert_untouched(&self) {
        let now = self.handle.status().await.unwrap();
        assert_eq!(now.events, self.before.events);
        assert_eq!(now.state_hash, self.before.state_hash);
        assert_eq!(now.waiting_on, self.before.waiting_on);
        assert_eq!(now.result, None, "somebody conceded");
    }

    async fn nobody_connected(&self) {
        let connected = self.handle.status().await.unwrap().connected;
        assert_eq!(connected, Vec::<PlayerId>::new());
    }
}

#[tokio::test]
async fn unknown_games_and_wrong_tokens_get_one_and_the_same_refusal() {
    let table = Table::start(config(), true).await;
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
        ..config()
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
    let table = Table::start(config(), false).await;
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
    let table = Table::start(config(), true).await;
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
async fn silence_after_connecting_is_closed_after_the_join_timeout() {
    let config = ServerConfig {
        socket: SocketLimits {
            join_timeout: Duration::from_millis(300),
            ..config().socket
        },
        ..config()
    };
    let table = Table::start(config, true).await;
    let mut client = net::connect(table.addr(), &table.game, &table.tokens[0]).await;
    assert_eq!(client.closed().await, Some(1008));
    table.nobody_connected().await;
    table.assert_untouched().await;
}

#[tokio::test]
async fn frames_over_the_limit_close_the_socket_and_never_reach_the_game() {
    let table = Table::start(config(), true).await;
    let max = SocketLimits::default().max_frame;
    let seat = table.waiting;
    let padded = |text: &str, size: usize| format!("{text}{}", " ".repeat(size - text.len()));

    // A frame of exactly the limit is accepted: Resync padded with blanks gets its snapshot.
    let mut exact = table.player(seat).await;
    table.expect_resync(&mut exact).await;
    exact.send_text(padded(r#"{"type":"Resync"}"#, max)).await;
    assert!(matches!(
        exact.message().await,
        ServerMessage::Snapshot { .. }
    ));
    exact.close().await;

    // One byte more is too much. The message is a valid Concede, and it must not be applied.
    let concede = r#"{"type":"Concede"}"#;
    for size in [max + 1, 5 * max] {
        let mut big = table.player(seat).await;
        big.send_text(padded(concede, size)).await;
        let code = big.closed().await;
        assert!(matches!(code, Some(1009 | 1002) | None), "{code:?}");
    }
    tokio::time::sleep(Duration::from_millis(100)).await;
    table.assert_untouched().await;
}

#[tokio::test]
async fn garbage_gets_an_error_each_and_five_in_a_row_close_the_socket() {
    let table = Table::start(config(), true).await;
    let seat = table.waiting;
    let mut client = table.player(seat).await;
    table.expect_resync(&mut client).await;

    let junk = [
        r#"{"type":"Concede"}x"#,
        r#"{"type":"Nope"}"#,
        r#"{"type":"Answer","decision":"seven","answer":null}"#,
        "{",
        "\u{0}\u{1}",
    ];
    let is_bad_message = |message: ServerMessage| {
        matches!(
            message,
            ServerMessage::Error {
                code: ErrorCode::BadMessage,
                ..
            }
        )
    };
    // Four bad messages, then a good one: the count starts again.
    for round in 0..2 {
        for text in &junk[..4] {
            client.send_text(*text).await;
            assert!(is_bad_message(client.message().await), "{text}");
        }
        if round == 0 {
            client.send(&ClientMessage::Resync).await;
            table.expect_resync(&mut client).await;
        }
    }
    // The fifth in a row is answered and then the socket is closed.
    client.send_text(junk[4]).await;
    assert!(is_bad_message(client.message().await));
    assert_eq!(client.closed().await, Some(1008));

    table.assert_untouched().await;
    // The seat can come back.
    let mut again = table.player(seat).await;
    table.expect_resync(&mut again).await;
}

#[tokio::test]
async fn a_message_cannot_name_a_seat_other_than_the_tokens() {
    let table = Table::start(config(), true).await;
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

#[tokio::test]
async fn a_flood_is_cut_off_by_the_rate_limit() {
    let config = ServerConfig {
        socket: SocketLimits {
            burst: 5,
            per_second: 1,
            ..config().socket
        },
        ..config()
    };
    let table = Table::start(config, true).await;
    let mut client = table.player(table.waiting).await;
    for _ in 0..50 {
        client.send(&ClientMessage::Resync).await;
    }
    assert_eq!(client.closed().await, Some(1008));
    table.assert_untouched().await;
}

#[tokio::test]
async fn a_client_that_answers_pings_stays_and_one_that_does_not_is_dropped() {
    let config = ServerConfig {
        socket: SocketLimits {
            ping_every: Duration::from_millis(100),
            idle_timeout: Duration::from_millis(500),
            ..config().socket
        },
        ..config()
    };
    let table = Table::start(config, false).await;

    // Seat 0 keeps reading, which answers every ping, and is still there after many timeouts.
    let mut alive = table.player(0).await;
    let stayed = tokio::time::timeout(Duration::from_millis(2_000), alive.closed()).await;
    assert!(
        stayed.is_err(),
        "a client that answers pings was dropped: {stayed:?}"
    );
    assert_eq!(
        table.handle.status().await.unwrap().connected,
        [PlayerId::from_index(0)]
    );

    // Seat 1 joins and then never reads, so it never pongs.
    let mut deaf = net::connect(table.addr(), &table.game, &table.tokens[1]).await;
    deaf.join(&table.game).await;
    // Seat 0 keeps reading (and so ponging) while the deaf seat runs out of time.
    let stayed = tokio::time::timeout(Duration::from_millis(1_500), alive.closed()).await;
    assert!(stayed.is_err(), "seat 0 was dropped: {stayed:?}");
    let connected = table.handle.status().await.unwrap().connected;
    assert_eq!(
        connected,
        [PlayerId::from_index(0)],
        "the deaf seat is dropped"
    );
    assert_eq!(deaf.closed().await, Some(1008));
}
