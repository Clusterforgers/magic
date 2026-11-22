//! Goal: the card support endpoints over real HTTP on Postgres: only a signed-in account reads them, an empty database answers empty, the overview carries the newest snapshot, the trend oldest first, the newest refresh and the feed newest first, the missing list belongs to the newest snapshot, and a server without a database says so.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::net;

use mtg_db::{
    Database, MissingCard, NewPrecon, NewSnapshot, RefreshReport, RefreshStatus, StateCounts,
};
use mtg_server::{CardSource, migrate};
use net::{PASSWORD, TestServer, bot_config, register, request};
use serde_json::Value;
use sqlx::PgPool;

const ROUTES: [&str; 2] = ["/api/card-support", "/api/card-support/missing"];

async fn start(pool: &PgPool) -> (TestServer, Database) {
    migrate(pool).await.unwrap();
    let db = Database::from_pool(pool.clone());
    db.migrate().await.unwrap();
    let server =
        TestServer::start_with_cards(pool.clone(), bot_config(), CardSource::new(pool.clone()))
            .await;
    (server, db)
}

fn snapshot(release: &str, automatic: u32, missing: &[&str], newly: &[&str]) -> NewSnapshot {
    NewSnapshot {
        release: Some(release.to_owned()),
        cards: 100,
        automatic,
        states: StateCounts {
            verified: automatic,
            manual: 100 - automatic,
            ..StateCounts::default()
        },
        buckets: Vec::new(),
        missing: missing
            .iter()
            .map(|name| MissingCard {
                oracle_id: format!("id-{name}"),
                name: (*name).to_owned(),
                state: "manual".to_owned(),
                blocker: "No rules yet".to_owned(),
                detail: "rules: no rules".to_owned(),
                blockers: 1,
                rank: None,
            })
            .collect(),
        newly_supported_count: u32::try_from(newly.len()).unwrap(),
        newly_supported: newly.iter().map(|n| (*n).to_owned()).collect(),
    }
}

#[tokio::test]
async fn a_server_without_a_database_counts_no_cards() {
    let server = TestServer::start(bot_config()).await;
    for path in ROUTES {
        let reply = request(server.addr, "GET", path, &[], b"").await;
        assert_eq!(reply.status, 503, "{path}");
        assert_eq!(reply.error_code(), "cards_unavailable", "{path}");
    }
}

#[sqlx::test(migrations = false)]
async fn card_support_needs_a_signed_in_account(pool: PgPool) {
    let (server, _) = start(&pool).await;
    for path in ROUTES {
        let reply = request(server.addr, "GET", path, &[], b"").await;
        assert_eq!(reply.status, 401, "{path}");
        assert_eq!(reply.error_code(), "unauthenticated", "{path}");
    }
}

#[sqlx::test(migrations = false)]
async fn nothing_stored_answers_empty(pool: PgPool) {
    let (server, _) = start(&pool).await;
    let ann = register(server.addr, "ann", PASSWORD).await;
    let overview = ann.get(server.addr, ROUTES[0]).await;
    assert_eq!(overview.status, 200, "{}", overview.text());
    let body = overview.json();
    assert_eq!(body["snapshot"], Value::Null);
    assert_eq!(body["last_check"], Value::Null);
    assert_eq!(body["history"], serde_json::json!([]));
    assert_eq!(body["feed"], serde_json::json!([]));
    let missing = ann.get(server.addr, ROUTES[1]).await.json();
    assert_eq!(missing["snapshot_id"], Value::Null);
    assert_eq!(missing["cards"], serde_json::json!([]));
}

#[sqlx::test(migrations = false)]
async fn the_overview_and_missing_list_follow_the_stored_rows(pool: PgPool) {
    let (server, db) = start(&pool).await;
    let feed = db.card_feed();
    feed.store_snapshot(&snapshot("v1", 90, &["A", "B"], &[]))
        .await
        .unwrap();
    let run = feed.start_refresh(Some("v1")).await.unwrap();
    let report = RefreshReport {
        mtgjson_build: Some("5.3.0+20261122".to_owned()),
        new_card_count: 2,
        new_cards: vec!["Kirk".into(), "Spock".into()],
        new_precons: vec![NewPrecon {
            code: "STC".into(),
            name: "Final Frontier".into(),
            set_name: Some("Star Trek Commander".into()),
        }],
        ..RefreshReport::default()
    };
    feed.finish_refresh(run, RefreshStatus::Succeeded, &report)
        .await
        .unwrap();
    // The times are kept to the second and the feed orders by them.
    tokio::time::sleep(std::time::Duration::from_millis(1_100)).await;
    feed.store_snapshot(&snapshot("v2", 91, &["B"], &["A"]))
        .await
        .unwrap();

    let ann = register(server.addr, "ann", PASSWORD).await;
    let body = ann.get(server.addr, ROUTES[0]).await.json();
    assert_eq!(body["snapshot"]["automatic"], 91);
    assert_eq!(body["snapshot"]["release"], "v2");
    assert_eq!(body["snapshot"]["missing"], 1);
    assert_eq!(body["snapshot"]["states"]["manual"], 9);
    let trend: Vec<u64> = body["history"]
        .as_array()
        .unwrap()
        .iter()
        .map(|point| point["automatic"].as_u64().unwrap())
        .collect();
    assert_eq!(trend, vec![90, 91]);
    assert_eq!(body["last_check"]["state"], "succeeded");
    assert_eq!(body["last_check"]["mtgjson_build"], "5.3.0+20261122");
    let kinds: Vec<&str> = body["feed"]
        .as_array()
        .unwrap()
        .iter()
        .map(|event| event["kind"].as_str().unwrap())
        .collect();
    assert_eq!(kinds, vec!["now_automatic", "new_precons", "new_cards"]);
    assert_eq!(body["feed"][0]["names"], serde_json::json!(["A"]));
    assert_eq!(body["feed"][1]["set_name"], "Star Trek Commander");

    let missing = ann.get(server.addr, ROUTES[1]).await.json();
    assert_eq!(missing["snapshot_id"], body["snapshot"]["id"]);
    assert_eq!(missing["cards"][0]["name"], "B");
    assert_eq!(missing["cards"][0]["state"], "manual");
    assert_eq!(missing["cards"][0]["blocker"], "No rules yet");
    assert_eq!(missing["cards"].as_array().unwrap().len(), 1);
}
