//! Goal: fix marks work end to end over real HTTP on Postgres: only a signed-in account reaches them and only its own, marking is idempotent and replaces the note, a note is trimmed and bounded, an unknown card is a plain 404, taking a mark back answers 204 once and 404 after, the ranking the owner reads counts accounts, deleting an account removes its marks, and a server without a database says so.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::net;

use mtg_db::Database;
use mtg_import::scryfall::cards;
use mtg_server::{CardSource, migrate};
use net::{Browser, PASSWORD, TestServer, bot_config, register, request};
use serde_json::{Value, json};
use sqlx::PgPool;

const UNKNOWN: &str = "00000000-0000-4000-8000-000000000000";

fn oracle(name: &str) -> String {
    let all: Vec<Value> =
        serde_json::from_str(&mtg_testkit::corpus::scryfall_array().unwrap()).unwrap();
    all.iter()
        .find(|card| card["name"] == name)
        .unwrap_or_else(|| panic!("the corpus has no {name}"))["oracle_id"]
        .as_str()
        .unwrap()
        .to_owned()
}

/// Both schemas migrated and the corpus imported, behind a server.
async fn start(pool: &PgPool) -> TestServer {
    migrate(pool).await.unwrap();
    let db = Database::from_pool(pool.clone());
    db.migrate().await.unwrap();
    db.importer()
        .import_cards(cards(
            mtg_testkit::corpus::scryfall_array().unwrap().as_bytes(),
        ))
        .await
        .unwrap();
    TestServer::start_with_cards(pool.clone(), bot_config(), CardSource::new(pool.clone())).await
}

async fn put(server: &TestServer, who: &Browser, id: &str, body: Value) -> net::Reply {
    who.put(
        server.addr,
        &format!("/api/fix-marks/{id}"),
        &body.to_string(),
    )
    .await
}

async fn marks(server: &TestServer, who: &Browser) -> Vec<Value> {
    let reply = who.get(server.addr, "/api/fix-marks").await;
    assert_eq!(reply.status, 200, "{}", reply.text());
    reply.json()["marks"].as_array().unwrap().clone()
}

#[tokio::test]
async fn a_server_without_a_database_has_no_marks() {
    let server = TestServer::start(bot_config()).await;
    let json_headers = [("Content-Type", "application/json")];
    for (method, path) in [
        ("GET", "/api/fix-marks".to_owned()),
        ("PUT", format!("/api/fix-marks/{UNKNOWN}")),
        ("DELETE", format!("/api/fix-marks/{UNKNOWN}")),
    ] {
        let reply = request(server.addr, method, &path, &json_headers, b"{}").await;
        assert_eq!(reply.status, 503, "{method} {path}");
        assert_eq!(reply.error_code(), "cards_unavailable", "{method} {path}");
    }
}

#[sqlx::test(migrations = false)]
async fn every_fix_mark_route_needs_a_signed_in_account(pool: PgPool) {
    let server = start(&pool).await;
    let json_headers = [("Content-Type", "application/json")];
    for (method, path) in [
        ("GET", "/api/fix-marks".to_owned()),
        ("PUT", format!("/api/fix-marks/{UNKNOWN}")),
        ("DELETE", format!("/api/fix-marks/{UNKNOWN}")),
    ] {
        let reply = request(server.addr, method, &path, &json_headers, b"{}").await;
        assert_eq!(reply.status, 401, "{method} {path}");
        assert_eq!(reply.error_code(), "unauthenticated", "{method} {path}");
    }
}

#[sqlx::test(migrations = false)]
async fn marking_is_idempotent_and_the_second_note_replaces_the_first(pool: PgPool) {
    let server = start(&pool).await;
    let ann = register(server.addr, "ann", PASSWORD).await;
    let bolt = oracle("Lightning Bolt");

    let first = put(&server, &ann, &bolt, json!({"note": "  deals 2  "})).await;
    assert_eq!(first.status, 200, "{}", first.text());
    let made = first.json();
    assert_eq!(made["oracle_id"], bolt);
    assert_eq!(made["name"], "Lightning Bolt");
    assert_eq!(made["note"], "deals 2");
    assert!(made["created_at"].as_str().unwrap().ends_with('Z'));

    let again = put(&server, &ann, &bolt, json!({})).await;
    assert_eq!(again.status, 200);
    assert_eq!(again.json()["note"], Value::Null);
    assert_eq!(again.json()["created_at"], made["created_at"]);
    assert_eq!(marks(&server, &ann).await.len(), 1);

    // An empty note is none, and the id may be spelled in capitals.
    let blank = put(&server, &ann, &bolt.to_uppercase(), json!({"note": "  "})).await;
    assert_eq!(blank.status, 200);
    assert_eq!(blank.json()["note"], Value::Null);
}

#[sqlx::test(migrations = false)]
async fn a_list_is_the_accounts_own_marks_newest_first(pool: PgPool) {
    let server = start(&pool).await;
    let ann = register(server.addr, "ann", PASSWORD).await;
    let bo = register(server.addr, "bob", PASSWORD).await;
    for name in ["Lightning Bolt", "Sol Ring"] {
        let reply = put(&server, &ann, &oracle(name), json!({})).await;
        assert_eq!(reply.status, 200);
    }
    put(&server, &bo, &oracle("Forest"), json!({"note": "bo's"})).await;

    let names: Vec<String> = marks(&server, &ann)
        .await
        .iter()
        .map(|mark| mark["name"].as_str().unwrap().to_owned())
        .collect();
    assert_eq!(names, ["Sol Ring", "Lightning Bolt"]);
    let theirs = marks(&server, &bo).await;
    assert_eq!(theirs.len(), 1);
    assert_eq!(theirs[0]["note"], "bo's");
    let cy = register(server.addr, "cy", PASSWORD).await;
    assert_eq!(marks(&server, &cy).await.len(), 0);
}

#[sqlx::test(migrations = false)]
async fn taking_a_mark_back_answers_204_once_and_never_touches_another_accounts(pool: PgPool) {
    let server = start(&pool).await;
    let ann = register(server.addr, "ann", PASSWORD).await;
    let bo = register(server.addr, "bob", PASSWORD).await;
    let ring = oracle("Sol Ring");
    put(&server, &ann, &ring, json!({})).await;
    put(&server, &bo, &ring, json!({})).await;

    let path = format!("/api/fix-marks/{ring}");
    assert_eq!(ann.delete(server.addr, &path).await.status, 204);
    let again = ann.delete(server.addr, &path).await;
    assert_eq!(
        (again.status, again.error_code().as_str()),
        (404, "not_found")
    );
    assert_eq!(marks(&server, &ann).await.len(), 0);
    assert_eq!(marks(&server, &bo).await.len(), 1, "bo's mark stays");
}

#[sqlx::test(migrations = false)]
async fn a_bad_card_or_note_is_refused_and_stores_nothing(pool: PgPool) {
    let server = start(&pool).await;
    let ann = register(server.addr, "ann", PASSWORD).await;
    let bolt = oracle("Lightning Bolt");

    for id in [UNKNOWN, "not-an-id"] {
        let reply = put(&server, &ann, id, json!({})).await;
        assert_eq!(
            (reply.status, reply.error_code().as_str()),
            (404, "not_found"),
            "{id}"
        );
        let reply = ann
            .delete(server.addr, &format!("/api/fix-marks/{id}"))
            .await;
        assert_eq!(reply.status, 404, "{id}");
    }
    for body in [
        json!({"note": "x".repeat(281)}),
        json!({"note": "bell\u{7}"}),
        json!({"note": 3}),
        json!({"text": "wrong member"}),
    ] {
        let reply = put(&server, &ann, &bolt, body.clone()).await;
        assert_eq!(reply.status, 400, "{body}");
        assert_eq!(reply.error_code(), "invalid_request", "{body}");
    }
    let reply = put(&server, &ann, &bolt, json!({"note": "é".repeat(280)})).await;
    assert_eq!(reply.status, 200, "280 characters fit: {}", reply.text());
    let not_json = request(
        server.addr,
        "PUT",
        &format!("/api/fix-marks/{bolt}"),
        &[("Cookie", &ann.cookie), ("Content-Type", "text/plain")],
        b"{}",
    )
    .await;
    assert_eq!(not_json.status, 415);
    assert_eq!(marks(&server, &ann).await.len(), 1);
}

#[sqlx::test(migrations = false)]
async fn the_ranking_the_owner_reads_counts_accounts_and_deleting_an_account_removes_its_marks(
    pool: PgPool,
) {
    let server = start(&pool).await;
    let ann = register(server.addr, "ann", PASSWORD).await;
    let bo = register(server.addr, "bob", PASSWORD).await;
    let ring = oracle("Sol Ring");
    put(&server, &ann, &ring, json!({"note": "ann's"})).await;
    put(&server, &bo, &ring, json!({"note": "bo's"})).await;
    put(&server, &ann, &oracle("Forest"), json!({})).await;

    let repo = Database::from_pool(pool.clone()).fix_marks();
    let ranked = repo.ranked(None).await.unwrap();
    assert_eq!(
        ranked
            .iter()
            .map(|mark| (mark.name.as_str(), mark.marks))
            .collect::<Vec<_>>(),
        [("Sol Ring", 2), ("Forest", 1)]
    );
    assert_eq!(ranked[0].newest_note.as_deref(), Some("bo's"));

    let body = json!({"password": PASSWORD}).to_string();
    let gone = ann
        .send(server.addr, "DELETE", "/api/auth/account", &body, &[])
        .await;
    assert_eq!(gone.status, 204, "{}", gone.text());
    let ranked = repo.ranked(None).await.unwrap();
    assert_eq!(ranked.len(), 1, "{ranked:?}");
    assert_eq!((ranked[0].name.as_str(), ranked[0].marks), ("Sol Ring", 1));
    assert_eq!(ranked[0].newest_note.as_deref(), Some("bo's"));
}
