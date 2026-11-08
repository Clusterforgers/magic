//! Goal: the read endpoints work end to end over real HTTP on Postgres: only a signed-in account within its budget reaches them, a deck lists what its reader has left to read, a card shows its draft's sheet and the account's verdict, `correct` approves the draft and `wrong` needs a note and marks the card to fix, a hash that is no longer current is 409 `stale_read`, the card endpoint carries the draft, and deleting an account takes its verdicts.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::net;

use mtg_cards::builtin;
use mtg_cards::load::{Library, Source, slug};
use mtg_cards::support::read_hash;
use mtg_db::{Database, DraftKey, NewDraft, SOURCE_SWEEP};
use mtg_import::ids::OracleId;
use mtg_import::scryfall::cards;
use mtg_server::{CardSource, migrate, printed_card};
use net::{Browser, PASSWORD, TestServer, bot_config, register, request};
use serde_json::{Value, json};
use sqlx::PgPool;

/// A card the corpus writes by hand; the tests keep its file out and read it as a draft instead.
const CARD: &str = "Explore";
const UNKNOWN: &str = "00000000-0000-4000-8000-000000000000";

fn id_of(name: &str) -> String {
    crate::common::oracle_id_of(name)
}

fn without_card() -> Library {
    let skipped = format!("cards/{}.json", slug(CARD));
    let sources: Vec<Source> = builtin::sources()
        .into_iter()
        .filter(|source| source.path != skipped)
        .collect();
    Library::from_sources(&sources).unwrap_or_else(|errors| panic!("{errors}"))
}

/// Both schemas migrated, the corpus imported and a pending draft of [`CARD`] stored; returns its key.
async fn seeded(pool: &PgPool) -> (Database, DraftKey) {
    migrate(pool).await.unwrap();
    let db = Database::from_pool(pool.clone());
    db.migrate().await.unwrap();
    db.importer()
        .import_cards(cards(
            mtg_testkit::corpus::scryfall_array().unwrap().as_bytes(),
        ))
        .await
        .unwrap();
    let card = builtin::library()
        .unwrap()
        .card_by_name(CARD)
        .unwrap()
        .clone();
    let stored = db.cards().get(&OracleId(id_of(CARD))).await.unwrap();
    let key = DraftKey {
        oracle_id: OracleId(id_of(CARD)),
        read_hash: read_hash(&printed_card(&stored.unwrap()), &card),
    };
    db.drafts()
        .insert(&[NewDraft {
            oracle_id: key.oracle_id.clone(),
            read_hash: key.read_hash.clone(),
            rules: serde_json::to_value(&card).unwrap(),
            source: SOURCE_SWEEP.to_owned(),
            oracle_matches: false,
            differs: false,
            proof: None,
        }])
        .await
        .unwrap();
    (db, key)
}

async fn start(pool: &PgPool) -> (Database, DraftKey, TestServer) {
    let (db, key) = seeded(pool).await;
    let source = CardSource::new(pool.clone()).with_rules(without_card());
    let server = TestServer::start_with_cards(pool.clone(), bot_config(), source).await;
    (db, key, server)
}

/// A stored deck of Forests and four of [`CARD`]; returns its id.
async fn store(server: &TestServer, who: &Browser) -> String {
    let line = |name: &str, count: u32| json!({"zone": "main", "oracle_id": id_of(name), "printing_id": null, "count": count});
    let body = json!({"name": "Explorers", "format": "freeform", "notes": "",
                      "cards": [line("Forest", 56), line(CARD, 4)]});
    let reply = who.post(server.addr, "/api/decks", &body.to_string()).await;
    assert_eq!(reply.status, 201, "{}", reply.text());
    reply.json()["id"].as_str().unwrap().to_owned()
}

async fn queue(server: &TestServer, who: &Browser, deck: &str) -> Vec<Value> {
    let reply = who
        .get(server.addr, &format!("/api/reads?deck={deck}"))
        .await;
    assert_eq!(reply.status, 200, "{}", reply.text());
    assert_eq!(reply.json()["title"], "Explorers");
    reply.json()["cards"].as_array().unwrap().clone()
}

async fn verdict(server: &TestServer, who: &Browser, body: Value) -> net::Reply {
    who.put(
        server.addr,
        &format!("/api/reads/{}", id_of(CARD)),
        &body.to_string(),
    )
    .await
}

async fn support(server: &TestServer) -> Value {
    let detail = net::get(server.addr, &format!("/api/cards/{}", id_of(CARD)))
        .await
        .json();
    detail["card"]["support"].clone()
}

#[tokio::test]
async fn a_server_without_a_database_has_nothing_to_read() {
    let server = TestServer::start(bot_config()).await;
    let json_headers = [("Content-Type", "application/json")];
    for (method, path) in [
        ("GET", "/api/reads?deck=x".to_owned()),
        ("GET", format!("/api/reads/{UNKNOWN}")),
        ("PUT", format!("/api/reads/{UNKNOWN}")),
    ] {
        let reply = request(server.addr, method, &path, &json_headers, b"{}").await;
        assert_eq!(reply.status, 503, "{method} {path}");
        assert_eq!(reply.error_code(), "cards_unavailable", "{method} {path}");
    }
}

#[sqlx::test(migrations = false)]
async fn every_read_route_needs_a_signed_in_account(pool: PgPool) {
    let (_, _, server) = start(&pool).await;
    let json_headers = [("Content-Type", "application/json")];
    for (method, path) in [
        ("GET", "/api/reads?precon=c18".to_owned()),
        ("GET", format!("/api/reads/{}", id_of(CARD))),
        ("PUT", format!("/api/reads/{}", id_of(CARD))),
    ] {
        let reply = request(server.addr, method, &path, &json_headers, b"{}").await;
        assert_eq!(reply.status, 401, "{method} {path}");
        assert_eq!(reply.error_code(), "unauthenticated", "{method} {path}");
    }
}

#[sqlx::test(migrations = false)]
async fn a_deck_lists_the_draft_and_the_card_shows_its_sheet(pool: PgPool) {
    let (_, key, server) = start(&pool).await;
    let ann = register(server.addr, "ann", PASSWORD).await;
    let deck = store(&server, &ann).await;

    let cards = queue(&server, &ann, &deck).await;
    assert_eq!(cards.len(), 1, "{cards:?}");
    assert_eq!(cards[0]["name"], CARD);
    assert_eq!(cards[0]["state"], "pending");
    assert_eq!(cards[0]["read_hash"], key.read_hash);

    let reply = ann
        .get(server.addr, &format!("/api/reads/{}", id_of(CARD)))
        .await;
    assert_eq!(reply.status, 200, "{}", reply.text());
    let card = reply.json();
    assert_eq!(card["read_hash"], key.read_hash);
    assert_eq!(card["state"], "pending");
    assert_eq!(card["mine"], Value::Null);
    let rows = card["sheet"]["rows"].as_array().unwrap();
    assert_ne!(rows.len(), 0);
    assert!(rows[0]["printed"].as_str().unwrap().contains("land"));
    assert_ne!(rows[0]["steps"].as_array().unwrap().len(), 0);

    let detail = net::get(server.addr, &format!("/api/cards/{}", id_of(CARD)))
        .await
        .json();
    assert_eq!(
        detail["draft"],
        json!({"read_hash": key.read_hash, "state": "pending"})
    );
    let bolt = net::get(server.addr, &format!("/api/cards/{}", id_of("Forest")))
        .await
        .json();
    assert!(
        bolt.get("draft").is_none(),
        "a card without a draft has none"
    );
}

#[sqlx::test(migrations = false)]
async fn a_correct_read_approves_the_draft_and_leaves_it_to_others(pool: PgPool) {
    let (_, key, server) = start(&pool).await;
    let ann = register(server.addr, "ann", PASSWORD).await;
    let bo = register(server.addr, "bob", PASSWORD).await;
    let deck = store(&server, &ann).await;
    assert_eq!(support(&server).await, "manual");

    let reply = verdict(
        &server,
        &ann,
        json!({"read_hash": key.read_hash, "verdict": "correct", "note": "ignored"}),
    )
    .await;
    assert_eq!(reply.status, 200, "{}", reply.text());
    let card = reply.json();
    assert_eq!(card["state"], "approved");
    assert_eq!(
        (card["correct"].as_u64(), card["wrong"].as_u64()),
        (Some(1), Some(0))
    );
    assert_eq!(card["mine"]["verdict"], "correct");
    assert_eq!(card["mine"]["note"], Value::Null);
    assert_eq!(card["mine"]["current"], true);
    assert_eq!(support(&server).await, "read");

    assert!(
        queue(&server, &ann, &deck).await.is_empty(),
        "ann judged it"
    );
    let theirs = bo
        .get(server.addr, &format!("/api/reads/{}", id_of(CARD)))
        .await
        .json();
    assert_eq!(theirs["state"], "approved");
    assert_eq!(theirs["mine"], Value::Null);
}

#[sqlx::test(migrations = false)]
async fn a_wrong_read_without_a_fit_note_is_refused(pool: PgPool) {
    let (db, key, server) = start(&pool).await;
    let ann = register(server.addr, "ann", PASSWORD).await;
    for mut body in [
        json!({"verdict": "wrong"}),
        json!({"verdict": "wrong", "note": "   "}),
        json!({"verdict": "wrong", "note": "a\u{7}b"}),
        json!({"verdict": "wrong", "note": "x".repeat(281)}),
        json!({"verdict": "perhaps"}),
    ] {
        body["read_hash"] = json!(key.read_hash);
        let reply = verdict(&server, &ann, body.clone()).await;
        assert_eq!(reply.status, 400, "{body}: {}", reply.text());
    }
    assert_eq!(db.reads().list(false).await.unwrap().len(), 0);
}

#[sqlx::test(migrations = false)]
async fn a_wrong_read_marks_the_card_to_fix_and_plays_it_by_hand(pool: PgPool) {
    let (db, key, server) = start(&pool).await;
    let ann = register(server.addr, "ann", PASSWORD).await;
    let reply = verdict(
        &server,
        &ann,
        json!({"read_hash": key.read_hash, "verdict": "wrong", "note": "  puts two lands  "}),
    )
    .await;
    assert_eq!(reply.status, 200, "{}", reply.text());
    let card = reply.json();
    assert_eq!(card["state"], "wrong");
    assert_eq!(card["mine"]["note"], "puts two lands");
    assert_eq!(support(&server).await, "manual");

    let marks = ann.get(server.addr, "/api/fix-marks").await.json();
    assert_eq!(marks["marks"][0]["name"], CARD);
    assert_eq!(marks["marks"][0]["note"], "puts two lands");
    assert_eq!(db.reads().list(true).await.unwrap().len(), 1);
}

#[sqlx::test(migrations = false)]
async fn a_hash_that_is_not_current_is_a_stale_read_and_a_card_without_a_draft_is_404(
    pool: PgPool,
) {
    let (db, _, server) = start(&pool).await;
    let ann = register(server.addr, "ann", PASSWORD).await;
    let stale = verdict(
        &server,
        &ann,
        json!({"read_hash": "0".repeat(32), "verdict": "correct"}),
    )
    .await;
    assert_eq!(stale.status, 409, "{}", stale.text());
    assert_eq!(stale.error_code(), "stale_read");
    assert_eq!(db.reads().list(false).await.unwrap().len(), 0);

    for path in [
        format!("/api/reads/{}", id_of("Forest")),
        format!("/api/reads/{UNKNOWN}"),
        "/api/reads/not-an-id".to_owned(),
        format!("/api/reads?deck={UNKNOWN}"),
        "/api/reads?precon=no-such-precon".to_owned(),
    ] {
        let reply = ann.get(server.addr, &path).await;
        assert_eq!(reply.status, 404, "{path}: {}", reply.text());
    }
    let neither = ann.get(server.addr, "/api/reads").await;
    assert_eq!(neither.status, 400);
}

#[sqlx::test(migrations = false)]
async fn an_account_over_its_budget_waits(pool: PgPool) {
    let (db, key) = seeded(&pool).await;
    drop(db);
    let mut config = bot_config();
    config.auth.limits.read_requests_per_user = 2;
    let source = CardSource::new(pool.clone()).with_rules(without_card());
    let server = TestServer::start_with_cards(pool.clone(), config, source).await;
    let ann = register(server.addr, "ann", PASSWORD).await;
    let path = format!("/api/reads/{}", key.oracle_id);
    for _ in 0..2 {
        assert_eq!(ann.get(server.addr, &path).await.status, 200);
    }
    let over = ann.get(server.addr, &path).await;
    assert_eq!(over.status, 429);
    assert_eq!(over.error_code(), "rate_limited");
    let bo = register(server.addr, "bob", PASSWORD).await;
    assert_eq!(
        bo.get(server.addr, &path).await.status,
        200,
        "a budget per account"
    );
}

#[sqlx::test(migrations = false)]
async fn deleting_an_account_takes_its_verdicts_and_the_draft_waits_again(pool: PgPool) {
    let (db, key, server) = start(&pool).await;
    let ann = register(server.addr, "ann", PASSWORD).await;
    let reply = verdict(
        &server,
        &ann,
        json!({"read_hash": key.read_hash, "verdict": "correct"}),
    )
    .await;
    assert_eq!(reply.status, 200);
    assert_eq!(support(&server).await, "read");

    let body = json!({"password": PASSWORD}).to_string();
    let gone = ann
        .send(server.addr, "DELETE", "/api/auth/account", &body, &[])
        .await;
    assert_eq!(gone.status, 204, "{}", gone.text());
    assert_eq!(db.reads().list(false).await.unwrap().len(), 0);
    assert_eq!(support(&server).await, "manual");
}
