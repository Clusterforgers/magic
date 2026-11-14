//! Goal: a party's list of runs leaves abandoned runs out while the database keeps them, and a run unchanged for
//! seven days with no live fight is abandoned when the list is read; a fresh run stays listed.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::{net, pg_commander};

use net::{Browser, PASSWORD, Reply, TestServer, register, request};
use serde_json::{Value, json};
use sqlx::PgPool;
use uuid::Uuid;

async fn call(
    server: &TestServer,
    method: &str,
    path: &str,
    who: &Browser,
    body: Option<Value>,
) -> Reply {
    let mut headers: Vec<(&str, &str)> = vec![("Cookie", who.cookie.as_str())];
    let body = body.map(|b| b.to_string()).unwrap_or_default();
    if !body.is_empty() {
        headers.push(("Content-Type", "application/json"));
    }
    request(server.addr, method, path, &headers, body.as_bytes()).await
}

async fn new_run(server: &TestServer, host: &Browser) -> String {
    let reply = call(
        server,
        "POST",
        "/api/delves",
        host,
        Some(json!({"heat": 0})),
    )
    .await;
    assert_eq!(reply.status, 201, "{}", reply.text());
    reply.json()["id"].as_str().unwrap().to_owned()
}

async fn status_of(pool: &PgPool, id: &str) -> String {
    sqlx::query_scalar("SELECT status FROM delves WHERE id = $1")
        .bind(Uuid::parse_str(id).unwrap())
        .fetch_one(pool)
        .await
        .unwrap()
}

#[sqlx::test(migrations = false)]
async fn abandoned_and_stale_runs_leave_the_list_but_stay_stored(pool: PgPool) {
    let server = pg_commander::start(&pool).await;
    let host = register(server.addr, "hostess", PASSWORD).await;
    let fresh = new_run(&server, &host).await;
    let gone = new_run(&server, &host).await;
    let stale = new_run(&server, &host).await;

    let abandon = call(
        &server,
        "POST",
        &format!("/api/delves/{gone}/act"),
        &host,
        Some(json!({"kind": "abandon"})),
    )
    .await;
    assert_eq!(abandon.status, 200, "{}", abandon.text());
    sqlx::query("UPDATE delves SET updated_at = now() - interval '8 days' WHERE id = $1")
        .bind(Uuid::parse_str(&stale).unwrap())
        .execute(&pool)
        .await
        .unwrap();

    let list = call(&server, "GET", "/api/delves", &host, None)
        .await
        .json();
    let ids: Vec<&str> = list["delves"]
        .as_array()
        .unwrap()
        .iter()
        .map(|run| run["id"].as_str().unwrap())
        .collect();
    assert_eq!(ids, vec![fresh.as_str()]);
    assert_eq!(status_of(&pool, &gone).await, "abandoned");
    assert_eq!(
        status_of(&pool, &stale).await,
        "abandoned",
        "the stale run was abandoned"
    );
    // The abandoned run is still readable by its party.
    let read = call(&server, "GET", &format!("/api/delves/{gone}"), &host, None).await;
    assert_eq!(read.status, 200);
    assert_eq!(read.json()["run"]["status"], "Abandoned");
}
