//! Goal: a signed-in learner has one practice game at a time even without naming the one before (another tab, the Learn page after the browser's Back): a new lesson ends the account's earlier practice game and nobody else's, and practice games are never stored, so nothing waits on them.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::net;
use crate::pg_admin::roomy_limits;

use mtg_db::Database;
use mtg_server::{GameId, migrate};
use net::{Browser, PASSWORD, TestServer, bot_config, register};
use serde_json::json;
use sqlx::PgPool;

async fn server(pool: &PgPool) -> TestServer {
    migrate(pool).await.unwrap();
    Database::from_pool(pool.clone()).migrate().await.unwrap();
    let mut config = net::windowed_config();
    config.auth.limits = roomy_limits();
    config.fixed_seed = bot_config().fixed_seed;
    TestServer::start_on(pool.clone(), config).await
}

/// Starts `lesson` for `browser`, naming no earlier game.
async fn start(server: &TestServer, browser: &Browser, lesson: &str) -> GameId {
    let body = json!({ "lesson": lesson }).to_string();
    let reply = browser.post(server.addr, "/api/tutorial", &body).await;
    assert_eq!(reply.status, 201, "{}", reply.text());
    reply.json()["game"].as_str().unwrap().parse().unwrap()
}

#[sqlx::test(migrations = false)]
async fn a_new_lesson_ends_the_accounts_earlier_practice_game_and_nobody_elses(pool: PgPool) {
    let server = server(&pool).await;
    let ann = register(server.addr, "Ann_1", PASSWORD).await;
    let bo = register(server.addr, "Bo_1", PASSWORD).await;
    let first = start(&server, &ann, "seat").await;
    let theirs = start(&server, &bo, "seat").await;
    let second = start(&server, &ann, "mana").await;
    assert!(
        server.registry.handle(first).is_none(),
        "Ann's first game ended"
    );
    let mut games = server.registry.practice_games();
    games.sort_by_key(ToString::to_string);
    let mut want = vec![second, theirs];
    want.sort_by_key(ToString::to_string);
    assert_eq!(games, want);
    let stored: i64 = sqlx::query_scalar("SELECT count(*) FROM games")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(
        stored, 0,
        "practice games are never stored, so a release never waits on one"
    );
}
