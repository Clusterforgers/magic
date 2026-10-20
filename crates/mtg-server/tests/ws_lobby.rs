//! Goal: over real sockets every seat in a lobby is sent the roster (names from whoever made the game, who is connected, who is ready, how many seats are still needed) right after its welcome and after each join, hang-up, Ready and `NotReady`; the REST lookup lists the same ready seats; the start is announced by the game and not by a roster; and a stranger or a socket that never joined is told nothing and is not on the list.
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;
mod net;

use std::time::Duration;

use mtg_core::ids::PlayerId;
use mtg_protocol::{ClientMessage, ErrorCode, LobbySeat, SeatDeck, ServerMessage};
use net::{
    Frame, TestServer, WsClient, bot_config, connect, create_game, get, refused, try_connect,
};
use serde_json::{Value, json};

const NAMES: [&str; 3] = ["Ada", "Bo", "Cy of the Many Long Names Indeed"];

/// A game of three named seats on a fresh server.
async fn three_seats() -> (TestServer, String, Vec<String>) {
    let server = TestServer::start(bot_config()).await;
    let seats: Vec<Value> = NAMES
        .iter()
        .map(|name| json!({ "name": name, "deck": "mono-green" }))
        .collect();
    let (game, tokens) =
        create_game(server.addr, json!({ "format": "normal", "seats": seats })).await;
    (server, game, tokens)
}

/// The roster the named seats send, from each seat's (connected, ready).
fn lobby(flags: [(bool, bool); 3]) -> ServerMessage {
    let seats = flags
        .iter()
        .enumerate()
        .map(|(index, (connected, ready))| LobbySeat {
            seat: PlayerId::from_index(index),
            name: NAMES[index].to_owned(),
            connected: *connected,
            ready: *ready,
            deck: Some(SeatDeck {
                name: "Mono Green Stompers".to_owned(),
                format: "normal".to_owned(),
                manual_cards: 0,
            }),
        })
        .collect();
    let needed = flags.iter().filter(|(_, ready)| !ready).count();
    ServerMessage::Lobby {
        seats,
        needed: u32::try_from(needed).unwrap(),
    }
}

/// Opens seat `seat`'s socket and joins; returns it with the welcome consumed.
async fn seated(server: &TestServer, game: &str, tokens: &[String], seat: usize) -> WsClient {
    let mut socket = connect(server.addr, game, &tokens[seat]).await;
    assert_eq!(socket.welcome(game).await.index(), seat);
    socket
}

/// Nothing may arrive within a moment.
async fn nothing_for(socket: &mut WsClient, wait: Duration) {
    if let Ok(frame) = tokio::time::timeout(wait, socket.next()).await {
        panic!("expected silence, got {frame:?}");
    }
}

async fn lookup(server: &TestServer, game: &str) -> Value {
    let reply = get(server.addr, &format!("/api/games/{game}")).await;
    assert_eq!(reply.status, 200);
    reply.json()
}

#[tokio::test]
async fn the_roster_follows_the_welcome_and_every_change_in_the_documented_wire_shape() {
    let (server, game, tokens) = three_seats().await;

    let mut ada = seated(&server, &game, &tokens, 0).await;
    assert_eq!(
        ada.message().await,
        lobby([(true, false), (false, false), (false, false)])
    );
    // The exact text on the wire: a flat tagged object, seats in seat order, names as given, and
    // the deck each seat was made with as three facts.
    let deck = json!({"name": "Mono Green Stompers", "format": "normal", "manual_cards": 0});
    assert_eq!(
        serde_json::from_str::<Value>(ada.texts.last().unwrap()).unwrap(),
        json!({
            "type": "Lobby",
            "seats": [
                {"seat": 0, "name": "Ada", "connected": true, "ready": false, "deck": deck},
                {"seat": 1, "name": "Bo", "connected": false, "ready": false, "deck": deck},
                {"seat": 2, "name": NAMES[2], "connected": false, "ready": false, "deck": deck},
            ],
            "needed": 3,
        })
    );

    let mut bo = seated(&server, &game, &tokens, 1).await;
    let both = lobby([(true, false), (true, false), (false, false)]);
    assert_eq!(bo.message().await, both);
    assert_eq!(
        ada.message().await,
        both,
        "the seat that was here hears of it"
    );

    ada.send(&ClientMessage::Ready).await;
    let ada_ready = lobby([(true, true), (true, false), (false, false)]);
    assert_eq!(ada.message().await, ada_ready);
    assert_eq!(bo.message().await, ada_ready);

    // Taking it back is a message of its own, and so is saying it twice.
    ada.send(&ClientMessage::NotReady).await;
    ada.send(&ClientMessage::NotReady).await;
    ada.send_text(r#"{"type":"NotReady"}"#).await;
    assert_eq!(ada.message().await, both);
    assert_eq!(bo.message().await, both);
    ada.send(&ClientMessage::Ready).await;
    bo.send(&ClientMessage::Ready).await;
    assert_eq!(ada.message().await, ada_ready);
    assert_eq!(bo.message().await, ada_ready);
    let both_ready = lobby([(true, true), (true, true), (false, false)]);
    assert_eq!(ada.message().await, both_ready);
    assert_eq!(bo.message().await, both_ready);
    nothing_for(&mut ada, Duration::from_millis(200)).await;
}

#[tokio::test]
async fn the_last_ready_starts_the_game_and_the_opening_events_follow_not_a_roster() {
    let (server, game, tokens) = three_seats().await;
    let mut sockets = Vec::new();
    for seat in 0..3 {
        sockets.push(seated(&server, &game, &tokens, seat).await);
    }
    for socket in &mut sockets {
        // Each socket was sent the roster of its own join and of the joins after it.
        let mut rosters = 0;
        loop {
            match tokio::time::timeout(Duration::from_millis(200), socket.next()).await {
                Ok(Frame::Message(ServerMessage::Lobby { .. })) => rosters += 1,
                Ok(other) => panic!("unexpected {other:?}"),
                Err(_) => break,
            }
        }
        assert!(rosters >= 1);
    }
    for socket in &mut sockets {
        socket.send(&ClientMessage::Ready).await;
    }
    for (seat, socket) in sockets.iter_mut().enumerate() {
        // The roster pushes for the first two Ready presses may still be on their way; the third starts the game.
        let mut flags = Vec::new();
        let batch = loop {
            match socket.message().await {
                ServerMessage::Lobby { seats, needed } => {
                    assert_eq!(
                        needed as usize,
                        seats.iter().filter(|s| !s.ready).count(),
                        "seat {seat}"
                    );
                    flags.push(needed);
                }
                ServerMessage::Events(batch) => break batch,
                other => panic!("seat {seat}: unexpected {other:?}"),
            }
        };
        assert_eq!(batch.from_seq, 0, "the opening events start at seq 0");
        // Two seats were ready when the third pressed it, and that press was answered by the game, not by a roster.
        assert_eq!(flags, [2, 1], "seat {seat}: seats still needed");
    }
    let view = lookup(&server, &game).await;
    assert_eq!(view["started"], true);
    assert_eq!(
        view["ready"],
        json!([0, 1, 2]),
        "a started game has every seat ready"
    );
}

#[tokio::test]
async fn a_hang_up_a_polite_close_and_a_return_are_each_seen_by_the_others() {
    let (server, game, tokens) = three_seats().await;
    let mut ada = seated(&server, &game, &tokens, 0).await;
    assert!(matches!(ada.message().await, ServerMessage::Lobby { .. }));
    let mut bo = seated(&server, &game, &tokens, 1).await;
    assert!(matches!(bo.message().await, ServerMessage::Lobby { .. }));
    assert!(matches!(ada.message().await, ServerMessage::Lobby { .. }));
    bo.send(&ClientMessage::Ready).await;
    assert!(matches!(ada.message().await, ServerMessage::Lobby { .. }));

    // Bo's connection dies without a word: no frame of any kind reaches the server.
    bo.hang_up();
    assert_eq!(
        ada.message().await,
        lobby([(true, false), (false, true), (false, false)]),
        "away, but still ready"
    );

    // Back on a new socket: connected again, and still ready, since nobody took it back.
    let mut bo = seated(&server, &game, &tokens, 1).await;
    let back = lobby([(true, false), (true, true), (false, false)]);
    assert_eq!(bo.message().await, back);
    assert_eq!(ada.message().await, back);

    // A polite close is the same to the others.
    bo.close().await;
    assert_eq!(
        ada.message().await,
        lobby([(true, false), (false, true), (false, false)])
    );
    nothing_for(&mut ada, Duration::from_millis(200)).await;
}

#[tokio::test]
async fn the_lookup_lists_the_ready_seats_before_any_socket_opens_and_after_it_closes() {
    let (server, game, tokens) = three_seats().await;
    let fresh = lookup(&server, &game).await;
    assert_eq!(fresh["ready"], json!([]));
    assert_eq!(fresh["connected"], json!([]));
    assert_eq!(fresh["started"], false);

    let mut ada = seated(&server, &game, &tokens, 0).await;
    let mut bo = seated(&server, &game, &tokens, 1).await;
    ada.send(&ClientMessage::Ready).await;
    bo.send(&ClientMessage::Ready).await;
    // The lookup is asked after the roster of the second Ready has been seen, so both agree.
    let mut latest = None;
    while latest != Some(lobby([(true, true), (true, true), (false, false)])) {
        if let ServerMessage::Lobby { .. } = ada.message().await {
            latest = Some(
                ada.texts
                    .last()
                    .map(|text| serde_json::from_str(text).unwrap())
                    .unwrap(),
            );
        }
    }
    let view = lookup(&server, &game).await;
    assert_eq!(view["ready"], json!([0, 1]));
    assert_eq!(view["connected"], json!([0, 1]));

    // Seats that went away keep their Ready, and the page that opens later sees it before its socket does.
    ada.hang_up();
    bo.hang_up();
    let mut view = lookup(&server, &game).await;
    for _ in 0..200 {
        if view["connected"] == json!([]) {
            break;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
        view = lookup(&server, &game).await;
    }
    assert_eq!(view["connected"], json!([]));
    assert_eq!(view["ready"], json!([0, 1]));
    assert_eq!(view["started"], false);

    // NotReady shows in the lookup as soon as it has been handled.
    let mut ada = seated(&server, &game, &tokens, 0).await;
    ada.send(&ClientMessage::NotReady).await;
    loop {
        if let ServerMessage::Lobby { seats, .. } = ada.message().await
            && !seats[0].ready
        {
            break;
        }
    }
    assert_eq!(lookup(&server, &game).await["ready"], json!([1]));
}

#[tokio::test]
async fn a_stranger_is_told_nothing_and_a_socket_that_never_joined_is_not_on_the_list() {
    let (server, game, tokens) = three_seats().await;
    let mut ada = seated(&server, &game, &tokens, 0).await;
    assert!(matches!(ada.message().await, ServerMessage::Lobby { .. }));

    // Wrong token, unknown game, and a token for another game are all refused before any message is exchanged.
    let wrong = "0".repeat(64);
    let (status, _) = refused(server.addr, &game, &wrong, None).await;
    assert_eq!(status, 404);
    let elsewhere = "11111111-2222-4333-8444-555555555555";
    let (status, _) = refused(server.addr, elsewhere, &tokens[1], None).await;
    assert_eq!(status, 404);
    assert!(
        try_connect(server.addr, &game, "not-a-token", None)
            .await
            .is_err()
    );

    // A socket with a real token that has not sent `Join` is no seat yet: it hears nothing...
    let mut lurker = connect(server.addr, &game, &tokens[2]).await;
    let mut bo = seated(&server, &game, &tokens, 1).await;
    assert_eq!(
        ada.message().await,
        lobby([(true, false), (true, false), (false, false)]),
        "the lurker is not listed, nor are the refused strangers"
    );
    assert!(matches!(bo.message().await, ServerMessage::Lobby { .. }));
    nothing_for(&mut lurker, Duration::from_millis(300)).await;
    assert_eq!(lookup(&server, &game).await["connected"], json!([0, 1]));

    // ... until it joins, which is when everyone hears of it.
    assert_eq!(lurker.welcome(&game).await.index(), 2);
    let all = lobby([(true, false), (true, false), (true, false)]);
    assert_eq!(lurker.message().await, all);
    assert_eq!(ada.message().await, all);
}

#[tokio::test]
async fn a_misspelt_not_ready_is_a_bad_message_and_changes_nothing() {
    let (server, game, tokens) = three_seats().await;
    let mut ada = seated(&server, &game, &tokens, 0).await;
    assert!(matches!(ada.message().await, ServerMessage::Lobby { .. }));
    ada.send(&ClientMessage::Ready).await;
    assert!(matches!(ada.message().await, ServerMessage::Lobby { .. }));
    for bad in [
        r#"{"type":"Notready"}"#,
        r#"{"type":"not_ready"}"#,
        r#"{"type":"Lobby","seats":[],"needed":0}"#,
    ] {
        ada.send_text(bad).await;
        assert!(
            matches!(
                ada.message().await,
                ServerMessage::Error {
                    code: ErrorCode::BadMessage,
                    ..
                }
            ),
            "{bad}"
        );
    }
    assert_eq!(lookup(&server, &game).await["ready"], json!([0]));
}
