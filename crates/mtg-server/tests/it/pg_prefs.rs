//! Goal: account preferences work end to end over real HTTP on Postgres: only a signed-in account reaches them and only its own, a new account reads the defaults, a saved update reads back sorted and per account, bad stops are refused with 400, a server without a database says so, and deleting an account removes its row.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::net;

use mtg_cards::Caps;
use mtg_db::Database;
use mtg_server::{CardSource, migrate};
use net::{Browser, PASSWORD, TestServer, bot_config, register, request};
use serde_json::{Value, json};
use sqlx::PgPool;

const ROUTE: &str = "/api/me/prefs";

async fn start(pool: &PgPool) -> TestServer {
    migrate(pool).await.unwrap();
    Database::from_pool(pool.clone()).migrate().await.unwrap();
    TestServer::start_with_cards(
        pool.clone(),
        bot_config(),
        CardSource::new(pool.clone()).with_caps(Caps::everything()),
    )
    .await
}

async fn save(server: &TestServer, who: &Browser, body: &Value) -> net::Reply {
    who.put(server.addr, ROUTE, &body.to_string()).await
}

fn stop(side: &str, step: &str, keep: bool) -> Value {
    json!({"side": side, "step": step, "keep": keep})
}

fn defaults() -> Value {
    json!({"default_stops": [], "end_turn_by_hand": false, "auto_choices": {"order_triggers": false, "assign_damage": false, "smart_stops": false}})
}

#[tokio::test]
async fn a_server_without_a_database_keeps_no_preferences() {
    let server = TestServer::start(bot_config()).await;
    for method in ["GET", "PUT"] {
        let reply = request(server.addr, method, ROUTE, &[], b"").await;
        assert_eq!(reply.status, 503, "{method}");
        assert_eq!(reply.error_code(), "cards_unavailable", "{method}");
    }
}

#[sqlx::test(migrations = false)]
async fn preferences_need_a_signed_in_account(pool: PgPool) {
    let server = start(&pool).await;
    for method in ["GET", "PUT"] {
        let reply = request(server.addr, method, ROUTE, &[], b"").await;
        assert_eq!(reply.status, 401, "{method}");
        assert_eq!(reply.error_code(), "unauthenticated", "{method}");
    }
}

#[sqlx::test(migrations = false)]
async fn a_new_account_reads_the_defaults_and_a_save_reads_back_per_account(pool: PgPool) {
    let server = start(&pool).await;
    let ann = register(server.addr, "ann", PASSWORD).await;
    let bo = register(server.addr, "bob", PASSWORD).await;
    let fresh = ann.get(server.addr, ROUTE).await;
    assert_eq!(fresh.status, 200, "{}", fresh.text());
    assert_eq!(fresh.json(), defaults());

    let body = json!({
        "default_stops": [stop("Opponent", "Upkeep", true), stop("Own", "Upkeep", true)],
        "end_turn_by_hand": true,
        "auto_choices": {"order_triggers": true, "assign_damage": false, "smart_stops": true}
    });
    let saved = save(&server, &ann, &body).await;
    assert_eq!(saved.status, 200, "{}", saved.text());
    let expected = json!({
        "default_stops": [stop("Own", "Upkeep", true), stop("Opponent", "Upkeep", true)],
        "end_turn_by_hand": true,
        "auto_choices": {"order_triggers": true, "assign_damage": false, "smart_stops": true}
    });
    assert_eq!(saved.json(), expected);
    assert_eq!(ann.get(server.addr, ROUTE).await.json(), expected);
    assert_eq!(bo.get(server.addr, ROUTE).await.json(), defaults());

    assert_eq!(save(&server, &ann, &defaults()).await.status, 200);
    assert_eq!(ann.get(server.addr, ROUTE).await.json(), defaults());
}

#[sqlx::test(migrations = false)]
async fn bad_stops_are_refused_and_change_nothing(pool: PgPool) {
    let server = start(&pool).await;
    let ann = register(server.addr, "ann", PASSWORD).await;
    for stops in [
        vec![stop("Own", "Upkeep", false)],
        vec![stop("Own", "Untap", true)],
        vec![stop("Own", "Upkeep", true), stop("Own", "Upkeep", true)],
    ] {
        let body = json!({"default_stops": stops, "end_turn_by_hand": true});
        assert_eq!(save(&server, &ann, &body).await.status, 400, "{stops:?}");
    }
    assert_eq!(
        ann.put(server.addr, ROUTE, "{\"nope\": 1}").await.status,
        400
    );
    assert_eq!(ann.get(server.addr, ROUTE).await.json(), defaults());
}

#[sqlx::test(migrations = false)]
async fn deleting_an_account_removes_its_row_and_only_its_row(pool: PgPool) {
    let server = start(&pool).await;
    let ann = register(server.addr, "ann", PASSWORD).await;
    let bo = register(server.addr, "bob", PASSWORD).await;
    let mut body = defaults();
    body["end_turn_by_hand"] = json!(true);
    save(&server, &ann, &body).await;
    save(&server, &bo, &body).await;

    let gone = ann
        .send(
            server.addr,
            "DELETE",
            "/api/auth/account",
            &json!({"password": PASSWORD}).to_string(),
            &[],
        )
        .await;
    assert_eq!(gone.status, 204, "{}", gone.text());
    let rows: i64 = sqlx::query_scalar("SELECT count(*) FROM user_prefs")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(rows, 1);
    assert_eq!(bo.get(server.addr, ROUTE).await.json(), body);
}
