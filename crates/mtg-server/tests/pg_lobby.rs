//! Goal: a lobby that survives a restart comes back with its seat names and with nobody ready, because readiness is never stored: the lookup says so, the roster of the first seat to return says so, and the seats can press Ready again and start the game.
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;
mod net;

use mtg_core::ids::PlayerId;
use mtg_protocol::{ClientMessage, LobbySeat, SeatDeck, ServerMessage};
use mtg_server::{ServerConfig, migrate};
use net::{TestServer, bot_config, connect, create_game, get, two_seats};
use serde_json::json;
use sqlx::PgPool;

fn seeded() -> ServerConfig {
    ServerConfig {
        fixed_seed: Some(3),
        ..bot_config()
    }
}

/// The decks of the two-seat game, which the roster names.
const DECKS: [&str; 2] = ["Mono Green Stompers", "Mono Red Giants"];

/// The roster of the two-seat game "Ann" and "Bo" from each seat's (connected, ready).
fn roster(flags: [(bool, bool); 2]) -> ServerMessage {
    let seats = flags
        .iter()
        .zip(["Ann", "Bo"])
        .enumerate()
        .map(|(index, ((connected, ready), name))| LobbySeat {
            seat: PlayerId::from_index(index),
            name: name.to_owned(),
            connected: *connected,
            ready: *ready,
            deck: Some(SeatDeck {
                name: DECKS[index].to_owned(),
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

#[sqlx::test(migrations = false)]
async fn a_resumed_lobby_asks_everybody_to_press_ready_again(pool: PgPool) {
    migrate(&pool).await.unwrap();
    let mut server = TestServer::start_on(pool.clone(), seeded()).await;
    let (game, tokens) = create_game(server.addr, two_seats()).await;

    // Ann is ready and Bo is not even here when the process dies.
    let mut ann = connect(server.addr, &game, &tokens[0]).await;
    ann.welcome(&game).await;
    assert_eq!(ann.message().await, roster([(true, false), (false, false)]));
    ann.send(&ClientMessage::Ready).await;
    assert_eq!(ann.message().await, roster([(true, true), (false, false)]));
    let before = get(server.addr, &format!("/api/games/{game}")).await.json();
    assert_eq!(before["ready"], json!([0]));
    ann.hang_up();
    server.shutdown();
    server.finished().await.unwrap();
    drop(server);

    let mut server = TestServer::start_on(pool.clone(), seeded()).await;
    let after = get(server.addr, &format!("/api/games/{game}")).await.json();
    assert_eq!(after["started"], false);
    assert_eq!(after["connected"], json!([]));
    assert_eq!(after["ready"], json!([]), "readiness is not stored");
    assert_eq!(
        after["seats"],
        json!([{"seat": 0, "name": "Ann"}, {"seat": 1, "name": "Bo"}])
    );

    // The old tokens seat the players again, the roster shows nobody ready, and the game starts when both say so.
    let mut ann = connect(server.addr, &game, &tokens[0]).await;
    ann.welcome(&game).await;
    assert_eq!(ann.message().await, roster([(true, false), (false, false)]));
    let mut bo = connect(server.addr, &game, &tokens[1]).await;
    bo.welcome(&game).await;
    let both_here = roster([(true, false), (true, false)]);
    assert_eq!(bo.message().await, both_here);
    assert_eq!(ann.message().await, both_here);
    ann.send(&ClientMessage::Ready).await;
    assert_eq!(ann.message().await, roster([(true, true), (true, false)]));
    assert_eq!(bo.message().await, roster([(true, true), (true, false)]));
    bo.send(&ClientMessage::Ready).await;
    for socket in [&mut ann, &mut bo] {
        let ServerMessage::Events(batch) = socket.message().await else {
            panic!("the opening events follow the last Ready, not a roster");
        };
        assert_eq!(batch.from_seq, 0);
    }
    let started = get(server.addr, &format!("/api/games/{game}")).await.json();
    assert_eq!(started["started"], true);
    assert_eq!(started["ready"], json!([0, 1]));
    server.shutdown();
    server.finished().await.unwrap();
}
