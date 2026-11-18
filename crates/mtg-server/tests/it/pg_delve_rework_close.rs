//! Goal: the boss rework's migration closes every open run (lobby or under way) with an `Abandoned` event and the
//! abandoned status, leaves finished and abandoned runs as they were, keeps every run stored, and can run again.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::{net, pg_commander};

use net::{PASSWORD, register, request};
use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;

const CLOSE: &str = include_str!("../../migrations/20261118000001_delves_rework_closes_runs.sql");

/// Each run's status and its last event, by id.
async fn runs(pool: &PgPool) -> Vec<(Uuid, String, Option<serde_json::Value>)> {
    sqlx::query_as(
        "SELECT d.id, d.status,
                (SELECT e.event FROM delve_events e WHERE e.delve_id = d.id ORDER BY e.seq DESC LIMIT 1)
         FROM delves d ORDER BY d.created_at, d.id",
    )
    .fetch_all(pool)
    .await
    .unwrap()
}

#[sqlx::test(migrations = false)]
async fn open_runs_close_and_finished_runs_stay(pool: PgPool) {
    let server = pg_commander::start(&pool).await;
    let host = register(server.addr, "hostess", PASSWORD).await;
    let headers = [
        ("Cookie", host.cookie.as_str()),
        ("Content-Type", "application/json"),
    ];
    let body = json!({"heat": 0}).to_string();
    for _ in 0..3 {
        let reply = request(
            server.addr,
            "POST",
            "/api/delves",
            &headers,
            body.as_bytes(),
        )
        .await;
        assert_eq!(reply.status, 201, "{}", reply.text());
    }
    let before = runs(&pool).await;
    // One run under way, one won and one lobby left open.
    for ((id, _, _), status) in before.iter().zip(["map", "victory", "lobby"]) {
        sqlx::query("UPDATE delves SET status = $2 WHERE id = $1")
            .bind(id)
            .bind(status)
            .execute(&pool)
            .await
            .unwrap();
    }
    for _ in 0..2 {
        sqlx::raw_sql(CLOSE).execute(&pool).await.unwrap();
    }
    let after = runs(&pool).await;
    assert_eq!(after.len(), 3);
    let abandoned = Some(json!("Abandoned"));
    assert_eq!(
        (after[0].1.as_str(), &after[0].2),
        ("abandoned", &abandoned)
    );
    assert_eq!(
        (after[1].1.as_str(), &after[1].2),
        ("victory", &before[1].2)
    );
    assert_eq!(
        (after[2].1.as_str(), &after[2].2),
        ("abandoned", &abandoned)
    );
    // A second run of the migration adds no second event.
    let events: i64 =
        sqlx::query_scalar("SELECT count(*) FROM delve_events WHERE event = '\"Abandoned\"'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(events, 2);
}
