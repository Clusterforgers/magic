//! Goal: over the real precons that `mtg-cli ingest` and `compile` stored (a database named by `MTG_REAL_DATABASE_URL`, never the development one), the list judges every precon by the same report as a stored deck, a precon is `playable` unless the report blocks it or the engine does not yet know a commander's rule (a planeswalker that can be a commander, a pair of partners of the newer kinds), every other precon makes a two-seat game that starts, and the precons that stay out are listed with their reasons. Ignored by default.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::net;

use std::collections::BTreeMap;

use mtg_server::{CardSource, ServerConfig, migrate};
use net::lobby;
use net::{TestServer, bot_config};
use serde_json::{Value, json};
use sqlx::PgPool;

/// The codes of the issues a precon is held back by.
fn codes(view: &Value) -> Vec<String> {
    view["issues"]
        .as_array()
        .unwrap()
        .iter()
        .map(|issue| issue["code"].as_str().unwrap().to_owned())
        .collect()
}

/// Run with
/// `MTG_REAL_DATABASE_URL=postgres://magic:magic@localhost:5432/<scratch> cargo test -p mtg-server --test it pg_commander_real:: -- --ignored --nocapture`
/// after `DATABASE_URL=<that database> cargo run -p mtg-cli -- ingest` and `-- compile`.
#[tokio::test]
#[ignore = "needs a database loaded with the real precons"]
async fn every_real_precon_is_judged_by_the_report_and_every_playable_one_starts_a_game() {
    let url =
        std::env::var("MTG_REAL_DATABASE_URL").expect("MTG_REAL_DATABASE_URL names a database");
    assert!(
        !url.ends_with("/magic"),
        "the development database is never used by tests"
    );
    let pool = PgPool::connect(&url).await.unwrap();
    migrate(&pool).await.unwrap();
    let config = ServerConfig {
        max_games: 1000,
        ..bot_config()
    };
    let cards = CardSource::new(pool.clone());
    let server = TestServer::start_with_cards(pool, config, cards).await;

    let list = net::get(server.addr, "/api/precons").await.json();
    let precons = list["precons"].as_array().unwrap();
    println!("{} precons", precons.len());
    assert!(!precons.is_empty(), "the database holds no precons");

    let mut by_state: BTreeMap<String, usize> = BTreeMap::new();
    let (mut playable, mut engine_waits, mut blocked) = (0, Vec::new(), Vec::new());
    for item in precons {
        let state = item["state"].as_str().unwrap().to_owned();
        *by_state.entry(state.clone()).or_default() += 1;
        if item["playable"] == true {
            playable += 1;
            continue;
        }
        let code = item["code"].as_str().unwrap();
        let view = net::get(server.addr, &format!("/api/precons/{code}"))
            .await
            .json();
        let issues = codes(&view);
        let line = format!("{code} [{state}] {}: {}", item["format"], issues.join(", "));
        if state == "blocked" {
            blocked.push(line);
        } else {
            // A precon the report passes is held back by the engine's check alone.
            let reasons = ["commander_eligibility", "commander_pair"];
            assert!(
                issues.iter().all(|issue| reasons.contains(&issue.as_str())),
                "{line}: {}",
                view["issues"]
            );
            engine_waits.push(format!("{line} ({})", view["issues"][0]["message"]));
        }
    }
    println!("states: {by_state:?}, playable: {playable}");
    for line in &blocked {
        println!("blocked by the check: {line}");
    }
    for line in &engine_waits {
        println!("waits for the engine: {line}");
    }
    assert_eq!(playable + blocked.len() + engine_waits.len(), precons.len());

    let mut started = 0;
    for item in precons.iter().filter(|item| item["playable"] == true) {
        let code = item["code"].as_str().unwrap();
        let format = if item["format"] == "commander" {
            "commander"
        } else {
            "normal"
        };
        let deck = format!("precon:{code}");
        let body = json!({"format": format, "held": 2, "seats": [
            {"name": "One", "deck": deck}, {"name": "Two", "deck": deck}]});
        let made = net::post_json(server.addr, "/api/games", &body.to_string()).await;
        assert_eq!(made.status, 201, "{code}: {}", made.text());
        let game = made.json()["game"].as_str().unwrap().to_owned();
        lobby::start(&server.registry, &game, 2).await;
        started += 1;
    }
    println!("{started} games started");
}
