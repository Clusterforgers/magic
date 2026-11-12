//! Goal: a player of a game hands out a watch link (`POST /api/games/{id}/spectators`); its socket (`/ws/{game}/watch/{token}`) holds no seat, is welcomed as the spectator viewer, gets the spectator's snapshot (no hand listed) and the spectators' batches and the result, never changes the game (a message is refused, a resync reconnects), and opens nothing for a stranger, a wrong token, a game not yet started or after the host stopped spectators (`PUT`, host only), which closes the spectators' sockets with 4001.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::net;

use mtg_core::ids::PlayerId;
use mtg_protocol::{ClientMessage, ErrorCode, ServerMessage};
use net::lobby::{bearer, call};
use net::table::{Table, table_config};
use net::{Reply, WsClient};
use serde_json::json;

fn path(game: &str) -> String {
    format!("/api/games/{game}/spectators")
}

/// `POST /api/games/{id}/spectators` presenting `token`.
async fn watch_link(table: &Table, token: &str) -> Reply {
    let auth = bearer(token);
    call(
        table.addr(),
        "POST",
        &path(&table.game),
        None,
        &[("Authorization", &auth)],
    )
    .await
}

/// `PUT /api/games/{id}/spectators` presenting `token`.
async fn allow(table: &Table, token: &str, allowed: bool) -> Reply {
    let auth = bearer(token);
    let body = json!({ "allowed": allowed }).to_string();
    call(
        table.addr(),
        "PUT",
        &path(&table.game),
        Some(&body),
        &[("Authorization", &auth)],
    )
    .await
}

/// A new watch link's token, handed out by the player in `seat`.
async fn link(table: &Table, seat: usize) -> String {
    let reply = watch_link(table, &table.tokens[seat]).await;
    assert_eq!(reply.status, 201, "{}", reply.text());
    let body = reply.json();
    assert_eq!(body["game"], table.game.as_str());
    body["token"].as_str().unwrap().to_owned()
}

fn watch_path(game: &str, token: &str) -> String {
    format!("/ws/{game}/watch/{token}")
}

/// Opens a spectator's socket and reads its welcome and snapshot.
async fn spectator(table: &Table, token: &str) -> WsClient {
    let mut client = net::open(table.addr(), &watch_path(&table.game, token), &[]).await;
    assert_eq!(
        client.welcome(&table.game).await,
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

#[tokio::test]
async fn a_spectator_sees_the_game_and_its_end_without_a_seat() {
    let table = Table::start(table_config(), true).await;
    let token = link(&table, 1).await;
    let mut watching = spectator(&table, &token).await;
    let status = table.handle.status().await.unwrap();
    assert_eq!(
        status.connected,
        Vec::<PlayerId>::new(),
        "a spectator is no seat"
    );

    table
        .handle
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
            _ => {}
        }
    }
    assert!(batch, "the concession reached the spectator as a batch");
}

#[tokio::test]
async fn a_spectator_changes_nothing_and_a_resync_reconnects() {
    let table = Table::start(table_config(), true).await;
    let token = link(&table, 0).await;
    let mut watching = spectator(&table, &token).await;
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
    table.assert_untouched().await;
    watching.send(&ClientMessage::Resync).await;
    assert_eq!(watching.closed().await, Some(1013));
    spectator(&table, &token).await;
}

#[tokio::test]
async fn watch_links_open_nothing_for_strangers_wrong_tokens_or_a_game_not_started() {
    let table = Table::start(table_config(), true).await;
    let stranger = call(table.addr(), "POST", &path(&table.game), None, &[]).await;
    // Without a seat token a session is asked for, and this server has no accounts.
    assert_eq!(stranger.status, 503, "{}", stranger.text());
    let forged = watch_link(&table, &"A".repeat(43)).await;
    assert_eq!(forged.status, 404);
    let token = link(&table, 0).await;
    let wrong = "B".repeat(token.len());
    for path in [
        watch_path(&table.game, &wrong),
        watch_path(&table.game, &table.tokens[0]),
        watch_path("not-a-game", &token),
        watch_path(&uuid::Uuid::new_v4().to_string(), &token),
    ] {
        let (status, _) = net::refused_at(table.addr(), &path, &[]).await;
        assert_eq!(status, 404, "{path}");
    }

    let lobby = Table::start(table_config(), false).await;
    let early = link(&lobby, 0).await;
    let mut socket = net::open(lobby.addr(), &watch_path(&lobby.game, &early), &[]).await;
    socket.join(&lobby.game).await;
    assert_eq!(
        socket.closed().await,
        Some(1001),
        "there is no table to watch yet"
    );
}

#[tokio::test]
async fn the_host_stops_spectators_and_allows_them_again() {
    let table = Table::start(table_config(), true).await;
    let token = link(&table, 1).await;
    let mut watching = spectator(&table, &token).await;

    let not_host = allow(&table, &table.tokens[1], false).await;
    assert_eq!(not_host.status, 403, "{}", not_host.text());
    let stopped = allow(&table, &table.tokens[0], false).await;
    assert_eq!(stopped.status, 200, "{}", stopped.text());
    assert_eq!(stopped.json(), json!({ "allowed": false }));
    assert_eq!(watching.closed().await, Some(4001));
    let info = call(
        table.addr(),
        "GET",
        &format!("/api/games/{}", table.game),
        None,
        &[],
    )
    .await;
    assert_eq!(info.json()["spectators"], false);

    let (status, _) = net::refused_at(table.addr(), &watch_path(&table.game, &token), &[]).await;
    assert_eq!(
        status, 404,
        "a link handed out before the stop opens nothing"
    );
    assert_eq!(watch_link(&table, &table.tokens[1]).await.status, 403);

    assert_eq!(allow(&table, &table.tokens[0], true).await.status, 200);
    let (status, _) = net::refused_at(table.addr(), &watch_path(&table.game, &token), &[]).await;
    assert_eq!(status, 404, "allowing again brings no old link back");
    let fresh = link(&table, 1).await;
    spectator(&table, &fresh).await;
}
