//! Goal: a shutdown signal closes every open socket with code 1001 (joined, silent, mid-game or deaf), stops the listener, and lets `serve` return cleanly within its grace period.
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;
mod net;

use std::time::{Duration, Instant};

use mtg_protocol::{ClientMessage, ServerMessage};
use net::{TestServer, bot_config};
use serde_json::json;

fn four_seats() -> serde_json::Value {
    let seats: Vec<_> = (0..4)
        .map(|i| json!({ "name": format!("P{i}"), "deck": "mono-white" }))
        .collect();
    json!({ "format": "normal", "seats": seats })
}

#[tokio::test]
async fn shutdown_says_going_away_to_every_socket_and_stops_listening() {
    let mut server = TestServer::start(bot_config()).await;
    let (game, tokens) = net::create_game(server.addr, four_seats()).await;

    // A seat in the lobby, a seat in a running game, one that connected but never joined, and one
    // that joined and then stopped reading altogether.
    let mut lobby = net::connect(server.addr, &game, &tokens[0]).await;
    assert_eq!(lobby.welcome(&game).await.index(), 0);
    let mut running = net::connect(server.addr, &game, &tokens[1]).await;
    assert_eq!(running.welcome(&game).await.index(), 1);
    let mut silent = net::connect(server.addr, &game, &tokens[2]).await;
    let mut deaf = net::connect(server.addr, &game, &tokens[3]).await;
    deaf.join(&game).await;
    for seat in 0..4 {
        let handle = server.registry.handle(game.parse().unwrap()).unwrap();
        let seat = mtg_core::ids::PlayerId::from_index(seat);
        handle.send(seat, ClientMessage::Ready).await.unwrap();
    }
    // Wait until the running game has shown seat 1 its opening events.
    loop {
        if matches!(running.message().await, ServerMessage::Events(_)) {
            break;
        }
    }

    let started = Instant::now();
    server.shutdown();
    assert_eq!(lobby.closed().await, Some(1001));
    assert_eq!(running.closed().await, Some(1001));
    assert_eq!(silent.closed().await, Some(1001));
    assert_eq!(deaf.closed().await, Some(1001));
    server.finished().await.unwrap();
    assert!(
        started.elapsed() < Duration::from_secs(10),
        "shutdown took {:?}",
        started.elapsed()
    );

    let refused = tokio::time::timeout(
        Duration::from_secs(5),
        tokio::net::TcpStream::connect(server.addr),
    )
    .await;
    assert!(
        matches!(refused, Ok(Err(_))),
        "the port still accepts connections"
    );
}

#[tokio::test]
async fn a_server_with_no_sockets_stops_at_once() {
    let mut server = TestServer::start(bot_config()).await;
    assert_eq!(net::get(server.addr, "/healthz").await.status, 200);
    let started = Instant::now();
    server.shutdown();
    server.finished().await.unwrap();
    assert!(
        started.elapsed() < Duration::from_secs(2),
        "{:?}",
        started.elapsed()
    );
}

#[tokio::test]
async fn a_socket_opened_while_the_server_stops_is_told_to_go() {
    let mut server = TestServer::start(bot_config()).await;
    let (game, tokens) = net::create_game(server.addr, four_seats()).await;
    // Upgrades that race the signal either lose to the 503 or get a 1001 close; none may linger.
    server.shutdown();
    for token in &tokens {
        if let Ok(mut client) = net::try_connect(server.addr, &game, token, None).await {
            assert_eq!(client.closed().await, Some(1001));
        }
    }
    server.finished().await.unwrap();
}
