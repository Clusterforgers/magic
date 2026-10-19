//! Goal: a player whose seat was taken away, because they left or the host removed them, has their socket closed with 4001, a code of its own that tells a client not to reconnect; the other seats stay connected and hear the seat open; a replaced socket still gets 4000, and a seat that is dropped but still held (a connection the server cut) still gets 1013 and may come back.
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;
mod net;

use mtg_core::ids::PlayerId;
use mtg_protocol::{ClientMessage, ServerMessage};
use net::lobby::{self, Made, call, join, post_as};
use net::{TestServer, WsClient, bot_config};
use serde_json::json;

/// A public game of three seats of which two guests, Bo and Cy, took the open ones.
struct Table {
    game: Made,
    bo: String,
    cy: String,
}

async fn table(server: &TestServer) -> Table {
    let game = lobby::make(server.addr, "Ann", 3, 1, "public").await;
    let mut tokens = Vec::new();
    for name in ["Bo", "Cy"] {
        let reply = join(server.addr, &game.id, &json!({ "name": name }), &[]).await;
        tokens.push(reply.json()["token"].as_str().unwrap().to_owned());
    }
    Table {
        game,
        bo: tokens[0].clone(),
        cy: tokens[1].clone(),
    }
}

/// Connects a seat and reads its `Welcome`.
async fn seated(server: &TestServer, game: &str, token: &str, seat: usize) -> WsClient {
    let mut socket = net::connect(server.addr, game, token).await;
    assert_eq!(socket.welcome(game).await, PlayerId::from_index(seat));
    socket
}

/// Reads until a roster arrives whose seat `seat` has `name`.
async fn roster_with(socket: &mut WsClient, seat: usize, name: &str) {
    loop {
        if let ServerMessage::Lobby { seats, .. } = socket.message().await
            && seats[seat].name == name
        {
            return;
        }
    }
}

#[tokio::test]
async fn the_host_removes_a_connected_player_and_that_socket_is_closed_with_4001() {
    let server = TestServer::start(bot_config()).await;
    let table = table(&server).await;
    let id = &table.game.id;
    let mut host = seated(&server, id, table.game.host(), 0).await;
    let mut bo = seated(&server, id, &table.bo, 1).await;
    let mut cy = seated(&server, id, &table.cy, 2).await;
    roster_with(&mut host, 1, "Bo").await;

    let path = format!("/api/games/{id}/seats/1/release");
    let removed = post_as(server.addr, &path, table.game.host()).await;
    assert_eq!(removed.status, 204, "{}", removed.text());

    assert_eq!(bo.closed().await, Some(4001));
    // The rest of the table hears the seat open and is not disturbed.
    roster_with(&mut host, 1, "").await;
    roster_with(&mut cy, 1, "").await;
    cy.send(&ClientMessage::Chat {
        text: "still here".into(),
    })
    .await;
    loop {
        if let ServerMessage::Chat { text, .. } = host.message().await {
            assert_eq!(text, "still here");
            break;
        }
    }
}

#[tokio::test]
async fn a_player_who_leaves_by_themselves_has_the_socket_closed_the_same_way() {
    let server = TestServer::start(bot_config()).await;
    let table = table(&server).await;
    let id = &table.game.id;
    let mut bo = seated(&server, id, &table.bo, 1).await;
    let mut cy = seated(&server, id, &table.cy, 2).await;

    let left = post_as(server.addr, &format!("/api/games/{id}/leave"), &table.bo).await;
    assert_eq!(left.status, 204, "{}", left.text());
    assert_eq!(bo.closed().await, Some(4001));
    roster_with(&mut cy, 1, "").await;
}

#[tokio::test]
async fn every_socket_of_the_removed_seat_is_closed_and_a_replaced_one_keeps_its_own_code() {
    let server = TestServer::start(bot_config()).await;
    let table = table(&server).await;
    let id = &table.game.id;
    let mut first = seated(&server, id, &table.bo, 1).await;
    // A second window takes the seat over; the first is told it was replaced.
    let mut second = seated(&server, id, &table.bo, 1).await;
    assert_eq!(first.closed().await, Some(4000));

    let removed = post_as(
        server.addr,
        &format!("/api/games/{id}/seats/1/release"),
        table.game.host(),
    )
    .await;
    assert_eq!(removed.status, 204);
    assert_eq!(second.closed().await, Some(4001));
}

#[tokio::test]
async fn a_removed_player_cannot_come_back_with_the_old_token_and_the_next_player_is_not_removed() {
    let server = TestServer::start(bot_config()).await;
    let table = table(&server).await;
    let id = &table.game.id;
    let mut bo = seated(&server, id, &table.bo, 1).await;
    let path = format!("/api/games/{id}/seats/1/release");
    post_as(server.addr, &path, table.game.host()).await;
    assert_eq!(bo.closed().await, Some(4001));

    // A client that obeyed 4001 does not retry; one that does is refused at the door.
    let (status, _) = net::refused(server.addr, id, &table.bo, None).await;
    assert_eq!(status, 404);

    // Someone else takes the seat and sits there undisturbed.
    let next = join(server.addr, id, &json!({ "name": "Di" }), &[]).await;
    let token = next.json()["token"].as_str().unwrap().to_owned();
    let mut di = seated(&server, id, &token, 1).await;
    di.send(&ClientMessage::Chat { text: "hi".into() }).await;
    loop {
        if let ServerMessage::Chat { from, .. } = di.message().await {
            assert_eq!(from, PlayerId(1));
            break;
        }
    }
}

#[tokio::test]
async fn a_seat_that_is_cut_but_still_held_is_told_to_reconnect_not_that_it_was_removed() {
    let server = TestServer::start(bot_config()).await;
    let table = table(&server).await;
    let id = &table.game.id;
    let mut bo = seated(&server, id, &table.bo, 1).await;
    // The server drops a connection without giving the seat up (a client too slow to keep up
    // is dropped the same way).
    let handle = server.registry.handle(id.parse().unwrap()).unwrap();
    handle.leave(PlayerId(1)).await;
    assert_eq!(bo.closed().await, Some(1013));
    // The token still works, so reconnecting is right.
    let mut again = seated(&server, id, &table.bo, 1).await;
    again
        .send(&ClientMessage::Chat {
            text: "back".into(),
        })
        .await;
    let reply = call(server.addr, "GET", &format!("/api/games/{id}"), None, &[]).await;
    assert_eq!(reply.json()["seats"][1]["name"], "Bo");
}
