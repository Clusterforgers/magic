//! Goal: `GET /api/games/{id}/replay` gives a finished recorded game as steps that fold into exactly the view the asker may have of the final state: the account that held a seat sees its own cards, any other account only what a spectator sees; a guest is refused (401) as for watching; an unknown, malformed or unfinished game is one 404.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::time::Duration;

use crate::common::{self, Client};
use crate::net::{self, PASSWORD, TestServer, bot_config, register};
use mtg_protocol::{ClientMessage, Replay, ServerMessage, StateView};
use mtg_server::{GameId, migrate};
use serde_json::json;
use sqlx::PgPool;

/// Plays seat 0 of `game` passively to the end while a bot plays seat 1.
async fn play_out(server: &TestServer, game: GameId) {
    let handle = server.registry.handle(game).unwrap();
    let mut me = Client::join(&handle, 0).await;
    me.say(&handle, ClientMessage::Ready).await;
    tokio::time::timeout(Duration::from_secs(600), async {
        loop {
            match me.inbox.recv().await.expect("the table closed seat 0") {
                ServerMessage::Decision { id, decision, .. } => {
                    let answer = common::passive(&decision);
                    me.say(
                        &handle,
                        ClientMessage::Answer {
                            decision: id,
                            answer,
                        },
                    )
                    .await;
                }
                ServerMessage::GameOver { .. } => return,
                _ => {}
            }
        }
    })
    .await
    .expect("the game ended");
}

/// The view a replay's steps fold into.
fn folded(replay: &Replay) -> StateView {
    let mut view = StateView::from_first_batch(&replay.steps[0]).unwrap();
    for step in &replay.steps[1..] {
        view.apply(step);
    }
    view
}

#[sqlx::test(migrations = false)]
async fn a_finished_game_replays_for_its_seat_and_in_public_for_anybody_else(pool: PgPool) {
    migrate(&pool).await.unwrap();
    let config = mtg_server::ServerConfig {
        bot_delay: Duration::from_millis(5),
        fixed_seed: Some(21),
        ..bot_config()
    };
    let server = TestServer::start_on(pool.clone(), config).await;
    let ann = register(server.addr, "Ann", PASSWORD).await;
    let bob = register(server.addr, "Bob", PASSWORD).await;
    let body = json!({"format": "normal", "held": 2, "seats": [
        {"name": "Ann", "deck": "mono-green"},
        {"deck": "mono-red", "bot": true},
    ]});
    let created = ann.post(server.addr, "/api/games", &body.to_string()).await;
    assert_eq!(created.status, 201, "{}", created.text());
    let game: GameId = created.json()["game"].as_str().unwrap().parse().unwrap();
    let path = format!("/api/games/{game}/replay");
    assert_eq!(
        ann.get(server.addr, &path).await.status,
        404,
        "not finished yet"
    );

    play_out(&server, game).await;
    // The record is marked finished just after the end; ask until it is.
    let mut mine = ann.get(server.addr, &path).await;
    for _ in 0..100 {
        if mine.status == 200 {
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
        mine = ann.get(server.addr, &path).await;
    }
    assert_eq!(mine.status, 200, "{}", mine.text());
    let mine: Replay = serde_json::from_slice(&mine.body).unwrap();
    assert_eq!(mine.seat, Some(mtg_core::ids::PlayerId::from_index(0)));
    assert_eq!(mine.names, ["Ann", "Bot"]);
    assert!(mine.result.is_some());
    let theirs: Replay = serde_json::from_slice(&bob.get(server.addr, &path).await.body).unwrap();
    let guest = net::get(server.addr, &path).await;
    assert_eq!(
        guest.status, 401,
        "a replay is for the signed-in, as watching is"
    );
    assert_eq!(theirs.seat, None);

    let status = server
        .registry
        .handle(game)
        .unwrap()
        .status()
        .await
        .unwrap();
    let mine_view = folded(&mine);
    let public_view = folded(&theirs);
    assert!(!mine_view.table.spectator);
    assert!(public_view.table.spectator);
    assert_eq!(mine.steps.len(), theirs.steps.len());
    let events: usize = mine.steps.iter().map(|step| step.events.len()).sum();
    assert_eq!(events as u64, status.events);
}

#[sqlx::test(migrations = false)]
async fn an_unknown_or_malformed_game_has_no_replay(pool: PgPool) {
    migrate(&pool).await.unwrap();
    let server = TestServer::start_on(pool, bot_config()).await;
    let ann = register(server.addr, "Ann", PASSWORD).await;
    let unknown = format!("/api/games/{}/replay", GameId::random());
    let guest = net::get(server.addr, &unknown).await;
    assert_eq!(guest.status, 401, "a guest is asked to sign in first");
    for path in [unknown, "/api/games/not-a-game/replay".to_owned()] {
        let reply = ann.get(server.addr, &path).await;
        assert_eq!(reply.status, 404, "{path}");
        assert_eq!(reply.json()["error"], "not_found");
    }
}
