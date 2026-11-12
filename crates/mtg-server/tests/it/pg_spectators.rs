//! Goal: any signed-in user watches any running game, public or invite, through `GET /ws/{game}/watch`: the socket holds no seat, is welcomed as the spectator viewer and gets the spectator's snapshot (no hand listed), batches and result, never changes the game (a message is refused, a resync reconnects); the seats are told how many watch as it changes and on joining; nobody signed out, no unknown game and no game not yet started is watched; and the game browser lists running games of either visibility as running after the public lobbies.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::net;

use mtg_core::ids::PlayerId;
use mtg_protocol::{ClientMessage, ErrorCode, ServerMessage};
use mtg_server::{GameHandle, migrate};
use net::{
    Browser, PASSWORD, TestServer, WsClient, bot_config, create_game, open, refused_at, register,
};
use serde_json::{Value, json};
use sqlx::PgPool;

async fn start(pool: &PgPool) -> TestServer {
    migrate(pool).await.unwrap();
    let mut config = bot_config();
    config.auth.limits.register_per_ip = 10_000;
    config.auth.limits.lobby_requests_per_ip = 10_000;
    TestServer::start_on(pool.clone(), config).await
}

/// A guest game of two seats with the given visibility; the handle, and its tokens.
async fn game(
    server: &TestServer,
    visibility: &str,
    started: bool,
) -> (String, Vec<String>, GameHandle) {
    let body = json!({ "format": "normal", "visibility": visibility, "seats": [
        { "name": "Ann", "deck": "mono-green" },
        { "name": "Bo", "deck": "mono-red" },
    ]});
    let (game, tokens) = create_game(server.addr, body).await;
    let handle = server.registry.handle(game.parse().unwrap()).unwrap();
    if started {
        for seat in [0, 1] {
            handle
                .send(PlayerId(seat), ClientMessage::Ready)
                .await
                .unwrap();
        }
        assert!(handle.status().await.unwrap().started);
    }
    (game, tokens, handle)
}

fn watch_path(game: &str) -> String {
    format!("/ws/{game}/watch")
}

/// Opens a spectator's socket as `browser` and reads its welcome and snapshot.
async fn spectator(server: &TestServer, game: &str, browser: &Browser) -> WsClient {
    let mut client = open(
        server.addr,
        &watch_path(game),
        &[("Cookie", &browser.cookie)],
    )
    .await;
    assert_eq!(
        client.welcome(game).await,
        PlayerId(2),
        "the spectator viewer, no seat"
    );
    match client.message().await {
        ServerMessage::Snapshot { state, .. } => {
            assert!(state.table.spectator);
            for player in &state.table.players {
                assert_eq!(player.hand.cards, None, "no hand is listed for a spectator");
            }
        }
        other => panic!("expected a snapshot, got {other:?}"),
    }
    client
}

/// A seat's socket, joined; of what it is sent first, only `Welcome` is read.
async fn seated(server: &TestServer, game: &str, token: &str) -> WsClient {
    let mut client = net::connect(server.addr, game, token).await;
    client.welcome(game).await;
    client
}

/// Reads until the next spectator count.
async fn count_of(client: &mut WsClient) -> u32 {
    loop {
        if let ServerMessage::Spectators { count } = client.message().await {
            return count;
        }
    }
}

#[sqlx::test(migrations = false)]
async fn a_signed_in_stranger_watches_an_invite_game_to_its_end(pool: PgPool) {
    let server = start(&pool).await;
    let (game, tokens, handle) = game(&server, "invite", true).await;
    let mut ann = seated(&server, &game, &tokens[0]).await;
    let eve = register(server.addr, "eve", PASSWORD).await;
    let mut watching = spectator(&server, &game, &eve).await;
    assert_eq!(
        count_of(&mut ann).await,
        1,
        "the seats are told someone watches"
    );
    assert_eq!(
        handle.status().await.unwrap().connected,
        [PlayerId(0)],
        "a spectator is no seat"
    );

    handle
        .send(PlayerId(0), ClientMessage::Concede)
        .await
        .unwrap();
    let mut batch = false;
    loop {
        match watching.message().await {
            ServerMessage::Events(events) => {
                assert!(events.table.spectator);
                batch = true;
            }
            ServerMessage::GameOver { .. } => break,
            ServerMessage::Spectators { .. } => panic!("a spectator is never told the count"),
            _ => {}
        }
    }
    assert!(batch, "the concession reached the spectator as a batch");
}

#[sqlx::test(migrations = false)]
async fn a_spectator_changes_nothing_and_a_resync_reconnects(pool: PgPool) {
    let server = start(&pool).await;
    let (game, _, handle) = game(&server, "public", true).await;
    let before = handle.status().await.unwrap();
    let eve = register(server.addr, "eve", PASSWORD).await;
    let mut watching = spectator(&server, &game, &eve).await;
    for message in [ClientMessage::Concede, ClientMessage::Ready] {
        watching.send(&message).await;
        loop {
            match watching.message().await {
                ServerMessage::Error { code, .. } => {
                    assert_eq!(code, ErrorCode::BadMessage);
                    break;
                }
                ServerMessage::Deciding { .. } => {}
                other => panic!("expected the refusal, got {other:?}"),
            }
        }
    }
    let after = handle.status().await.unwrap();
    assert_eq!(
        (after.events, after.state_hash, after.result),
        (before.events, before.state_hash, None)
    );
    watching.send(&ClientMessage::Resync).await;
    assert_eq!(watching.closed().await, Some(1013));
    spectator(&server, &game, &eve).await;
}

#[sqlx::test(migrations = false)]
async fn the_seats_hear_the_count_change_and_a_joining_seat_hears_it_too(pool: PgPool) {
    let server = start(&pool).await;
    let (game, tokens, _) = game(&server, "public", true).await;
    let mut ann = seated(&server, &game, &tokens[0]).await;
    let (eve, cy) = (
        register(server.addr, "eve", PASSWORD).await,
        register(server.addr, "cyd", PASSWORD).await,
    );
    let mut first = spectator(&server, &game, &eve).await;
    assert_eq!(count_of(&mut ann).await, 1);
    let _second = spectator(&server, &game, &cy).await;
    assert_eq!(count_of(&mut ann).await, 2);
    let mut bo = seated(&server, &game, &tokens[1]).await;
    assert_eq!(
        count_of(&mut bo).await,
        2,
        "a seat that joins hears how many watch"
    );
    first.close().await;
    assert_eq!(count_of(&mut ann).await, 1);
    assert_eq!(count_of(&mut bo).await, 1);
}

#[sqlx::test(migrations = false)]
async fn nobody_signed_out_no_unknown_game_and_no_lobby_is_watched(pool: PgPool) {
    let server = start(&pool).await;
    let (running, _, _) = game(&server, "public", true).await;
    let (status, body) = refused_at(server.addr, &watch_path(&running), &[]).await;
    assert_eq!(status, 401, "{body}");
    let eve = register(server.addr, "eve", PASSWORD).await;
    let cookie = [("Cookie", eve.cookie.as_str())];
    for path in [
        watch_path("not-a-game"),
        watch_path(&uuid::Uuid::new_v4().to_string()),
    ] {
        let (status, _) = refused_at(server.addr, &path, &cookie).await;
        assert_eq!(status, 404, "{path}");
    }
    let (lobby, _, _) = game(&server, "public", false).await;
    let mut socket = open(server.addr, &watch_path(&lobby), &cookie).await;
    socket.join(&lobby).await;
    assert_eq!(
        socket.closed().await,
        Some(1001),
        "there is no table to watch yet"
    );
}

#[sqlx::test(migrations = false)]
async fn the_game_browser_lists_running_games_of_either_visibility_beside_public_lobbies(
    pool: PgPool,
) {
    let server = start(&pool).await;
    let (public_lobby, _, _) = game(&server, "public", false).await;
    let (invite_lobby, _, _) = game(&server, "invite", false).await;
    let (public_running, _, _) = game(&server, "public", true).await;
    let (invite_running, _, handle) = game(&server, "invite", true).await;
    let (ended, _, ended_handle) = game(&server, "public", true).await;
    ended_handle
        .send(PlayerId(0), ClientMessage::Concede)
        .await
        .unwrap();
    assert!(ended_handle.status().await.unwrap().result.is_some());
    drop(handle);
    let reply = net::get(server.addr, "/api/lobbies").await;
    assert_eq!(reply.status, 200, "{}", reply.text());
    let rows: Vec<(String, bool)> = reply.json()["games"]
        .as_array()
        .unwrap()
        .iter()
        .map(|row: &Value| {
            (
                row["game"].as_str().unwrap().to_owned(),
                row.get("running").is_some_and(|flag| flag == &json!(true)),
            )
        })
        .collect();
    assert!(rows.contains(&(public_lobby, false)));
    assert!(rows.contains(&(public_running, true)));
    assert!(
        rows.contains(&(invite_running, true)),
        "any running game may be watched"
    );
    assert!(
        rows.iter().all(|(game, _)| *game != invite_lobby),
        "an invite lobby stays unlisted"
    );
    assert!(
        rows.iter().all(|(game, _)| *game != ended),
        "an ended game is not listed"
    );
}
