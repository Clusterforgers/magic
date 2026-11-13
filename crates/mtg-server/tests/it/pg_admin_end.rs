//! Goal: an admin ends any lobby or running game over real HTTP on Postgres: the record becomes `aborted` with the reason and no result, every connected seat and spectator is told `EndedByAdmin` with the reason before its socket closes with 4003, bot seats stop with the actor and nothing more is stored, the game leaves the registry and the admin's list; others get 401 or 403 before the body is read; a repeat, or a game that already ended, is a harmless 204; and the list shows lobbies and running games with their seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::net;
use crate::pg_admin::roomy_limits;

use std::time::Duration;

use mtg_core::ids::PlayerId;
use mtg_db::Database;
use mtg_protocol::{ClientMessage, ServerMessage};
use mtg_server::{ServerConfig, migrate};
use net::{Browser, PASSWORD, Reply, TestServer, WsClient, bot_config, login, open, register};
use serde_json::{Value, json};
use sqlx::PgPool;

const ADMIN: &str = "Chrisser";

async fn start_with(pool: &PgPool, change: impl FnOnce(&mut ServerConfig)) -> TestServer {
    migrate(pool).await.unwrap();
    Database::from_pool(pool.clone()).migrate().await.unwrap();
    let mut config = bot_config();
    config.auth.limits = roomy_limits();
    change(&mut config);
    TestServer::start_on(pool.clone(), config).await
}

/// A server whose admin is `ADMIN`, with the admin and `Friend_1` signed in.
async fn world(pool: &PgPool) -> (TestServer, Browser, Browser) {
    let first = start_with(pool, |_| {}).await;
    for name in [ADMIN, "Friend_1"] {
        register(first.addr, name, PASSWORD).await;
    }
    drop(first);
    let server = start_with(pool, |config| config.auth.admins = vec![ADMIN.to_owned()]).await;
    let admin = login(server.addr, ADMIN, PASSWORD).await;
    let friend = login(server.addr, "Friend_1", PASSWORD).await;
    (server, admin, friend)
}

fn end_path(game: &str) -> String {
    format!("/api/admin/games/{game}/end")
}

async fn end(by: &Browser, server: &TestServer, game: &str, body: &Value) -> Reply {
    by.post(server.addr, &end_path(game), &body.to_string())
        .await
}

/// The stored status, reason and result of `game`.
async fn record(pool: &PgPool, game: &str) -> (String, Option<String>, Option<Value>) {
    sqlx::query_as("SELECT status, end_reason, result FROM games WHERE id = $1::uuid")
        .bind(game)
        .fetch_one(pool)
        .await
        .unwrap()
}

async fn events(pool: &PgPool, game: &str) -> i64 {
    sqlx::query_scalar("SELECT count(*) FROM game_events WHERE game_id = $1::uuid")
        .bind(game)
        .fetch_one(pool)
        .await
        .unwrap()
}

/// A guest game: Ann at seat 0 and, with `bot`, a bot at seat 1 (else Bo); answers its id and tokens.
async fn game(server: &TestServer, bot: bool) -> (String, Vec<String>) {
    let second = if bot {
        json!({ "name": "Bot", "deck": "mono-red", "bot": true })
    } else {
        json!({ "name": "Bo", "deck": "mono-red" })
    };
    let body = json!({ "format": "normal", "visibility": "public", "seats": [
        { "name": "Ann", "deck": "mono-green" }, second,
    ]});
    net::create_game(server.addr, body).await
}

/// Reads until `EndedByAdmin`, which must come, and then the close code.
async fn told_and_closed(client: &mut WsClient) -> (Option<String>, Option<u16>) {
    loop {
        match client.message().await {
            ServerMessage::EndedByAdmin { reason } => return (reason, client.closed().await),
            ServerMessage::GameOver { .. } => panic!("an ended game has no result"),
            _ => {}
        }
    }
}

#[sqlx::test(migrations = false)]
async fn an_admin_ends_a_running_game_with_a_bot_and_a_spectator(pool: PgPool) {
    let (server, admin, friend) = world(&pool).await;
    let (game, tokens) = game(&server, true).await;
    let mut ann = net::connect(server.addr, &game, &tokens[0]).await;
    ann.welcome(&game).await;
    ann.send(&ClientMessage::Ready).await;
    let handle = server.registry.handle(game.parse().unwrap()).unwrap();
    for _ in 0..200 {
        if handle.status().await.unwrap().started {
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    assert!(handle.status().await.unwrap().started, "the bot readied");
    let mut watcher = open(
        server.addr,
        &format!("/ws/{game}/watch"),
        &[("Cookie", &friend.cookie)],
    )
    .await;
    watcher.welcome(&game).await;

    let listed = admin.get(server.addr, "/api/admin/games").await;
    assert_eq!(listed.status, 200, "{}", listed.text());
    let row = listed.json()["games"][0].clone();
    assert_eq!(row["game"], game.as_str());
    assert_eq!(row["started"], true);
    assert_eq!(row["format"], "normal");
    assert_eq!(row["seats"][0], json!({"name": "Ann", "connected": true}));
    assert_eq!(row["seats"][1]["name"], "Bot");

    let reply = end(
        &admin,
        &server,
        &game,
        &json!({"reason": "  server upkeep "}),
    )
    .await;
    assert_eq!(reply.status, 204, "{}", reply.text());
    for client in [&mut ann, &mut watcher] {
        let (reason, code) = told_and_closed(client).await;
        assert_eq!(reason.as_deref(), Some("server upkeep"));
        assert_eq!(code, Some(4003));
    }
    assert_eq!(
        record(&pool, &game).await,
        ("aborted".to_owned(), Some("server upkeep".to_owned()), None)
    );
    assert!(server.registry.handle(game.parse().unwrap()).is_none());
    assert!(handle.status().await.is_err(), "the actor stopped");
    // The bot seat plays no more: the log stays as it is.
    let stored = events(&pool, &game).await;
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert_eq!(events(&pool, &game).await, stored);
    let listed = admin.get(server.addr, "/api/admin/games").await.json();
    assert_eq!(listed["games"], json!([]));

    // A repeat changes nothing.
    let again = end(&admin, &server, &game, &json!({"reason": "twice"})).await;
    assert_eq!(again.status, 204, "{}", again.text());
    assert_eq!(
        record(&pool, &game).await.1.as_deref(),
        Some("server upkeep")
    );
}

#[sqlx::test(migrations = false)]
async fn an_admin_ends_a_lobby_and_a_finished_game_stays_finished(pool: PgPool) {
    let (server, admin, _) = world(&pool).await;
    let (lobby, tokens) = game(&server, false).await;
    let mut ann = net::connect(server.addr, &lobby, &tokens[0]).await;
    ann.welcome(&lobby).await;
    let listed = admin.get(server.addr, "/api/admin/games").await.json();
    assert_eq!(listed["games"][0]["started"], false);
    let reply = end(&admin, &server, &lobby, &json!({})).await;
    assert_eq!(reply.status, 204, "{}", reply.text());
    assert_eq!(told_and_closed(&mut ann).await, (None, Some(4003)));
    assert_eq!(
        record(&pool, &lobby).await,
        ("aborted".to_owned(), None, None)
    );
    let gone = net::lobby::lobbies(server.addr, &[]).await;
    assert!(!net::lobby::listed(&gone).contains(&lobby));

    let (done, _) = game(&server, false).await;
    let handle = server.registry.handle(done.parse().unwrap()).unwrap();
    for seat in [0, 1] {
        handle
            .send(PlayerId(seat), ClientMessage::Ready)
            .await
            .unwrap();
    }
    handle
        .send(PlayerId(0), ClientMessage::Concede)
        .await
        .unwrap();
    assert!(handle.status().await.unwrap().result.is_some());
    let reply = end(&admin, &server, &done, &json!({"reason": "late"})).await;
    assert_eq!(reply.status, 204, "{}", reply.text());
    assert_eq!(record(&pool, &done).await.0, "finished");
    assert!(server.registry.handle(done.parse().unwrap()).is_some());
}

#[sqlx::test(migrations = false)]
async fn only_an_admin_ends_a_game_and_the_body_is_checked(pool: PgPool) {
    let (server, admin, friend) = world(&pool).await;
    let (game, _) = game(&server, false).await;
    let body = json!({"reason": "mine"});
    let signed_out = net::lobby::call(
        server.addr,
        "POST",
        &end_path(&game),
        Some(&body.to_string()),
        &[],
    )
    .await;
    assert_eq!(signed_out.status, 401);
    let refused = end(&friend, &server, &game, &body).await;
    assert_eq!(
        (refused.status, refused.error_code().as_str()),
        (403, "forbidden")
    );
    // Refused before the body is read: even a broken one gets 403.
    let broken = friend.post(server.addr, &end_path(&game), "{").await;
    assert_eq!(broken.status, 403);
    assert_eq!(
        friend.get(server.addr, "/api/admin/games").await.status,
        403
    );
    assert_eq!(record(&pool, &game).await.0, "lobby");

    let long = json!({"reason": "x".repeat(201)});
    assert_eq!(end(&admin, &server, &game, &long).await.status, 400);
    let unknown = json!({"reason": "a", "winner": 0});
    assert_eq!(end(&admin, &server, &game, &unknown).await.status, 400);
    let missing = end(
        &admin,
        &server,
        "00000000-0000-4000-8000-000000000000",
        &body,
    )
    .await;
    assert_eq!(missing.status, 404);
    assert_eq!(end(&admin, &server, "not-a-game", &body).await.status, 404);
    assert_eq!(record(&pool, &game).await.0, "lobby");
}
