//! Goal: stored decks work end to end over real HTTP on Postgres: only a signed-in account reaches them and only its own, a save runs the deck check and stores its hashes, the Verified marker is derived each time (a clean check of an unchanged deck is verified; an edit, other rules or another engine remove it, a ban turns it into blocked, and a check that was not clean never gives it), the check endpoint reports without saving, a pasted list resolves to cards or is reported line by line, exports read back through the importer, the caps and rules of a deck are enforced, and a server without a database says so.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::net;

use std::sync::OnceLock;

use mtg_cards::Caps;
use mtg_cards::builtin;
use mtg_cards::load::Library;
use mtg_db::{Database, DeckId, DeckLine, DeckZone};
use mtg_import::ids::OracleId;
use mtg_import::scryfall::cards;
use mtg_server::{CardSource, migrate};
use net::{Browser, PASSWORD, TestServer, bot_config, get, register, request};
use serde_json::{Value, json};
use sqlx::PgPool;
use uuid::Uuid;

/// The Scryfall objects (trimmed, without legalities) of the corpus cards: the fixture files as one JSON array.
fn scryfall() -> String {
    mtg_testkit::corpus::scryfall_array().unwrap()
}

fn objects() -> &'static Vec<Value> {
    static OBJECTS: OnceLock<Vec<Value>> = OnceLock::new();
    OBJECTS.get_or_init(|| serde_json::from_str(&scryfall()).unwrap())
}

fn object(name: &str) -> &'static Value {
    objects()
        .iter()
        .find(|card| card["name"] == name)
        .unwrap_or_else(|| panic!("the corpus has no {name}"))
}

/// The oracle id of a corpus card.
fn oracle(name: &str) -> String {
    object(name)["oracle_id"].as_str().unwrap().to_owned()
}

/// The printing the corpus fixture holds of a card.
fn printing(name: &str) -> String {
    object(name)["id"].as_str().unwrap().to_owned()
}

/// Legalities the tests assign: the corpus is trimmed of them. Everything is legal in the four
/// formats the tests use, except where a test needs a card that is not.
fn legalities(name: &str) -> Value {
    let mut formats = json!({
        "commander": "legal", "modern": "legal", "legacy": "legal", "vintage": "legal"
    });
    if name == "Sol Ring" {
        formats["modern"] = json!("not_legal");
        formats["legacy"] = json!("banned");
        formats["vintage"] = json!("restricted");
    }
    formats
}

/// The corpus with legalities, and a token that no deck may hold.
fn fixture() -> String {
    let mut all: Vec<Value> = objects().clone();
    for card in &mut all {
        let name = card["name"].as_str().unwrap().to_owned();
        card["legalities"] = legalities(&name);
    }
    let mut token = object("Forest").clone();
    token["layout"] = json!("token");
    token["name"] = json!("Goblin");
    token["set"] = json!("tmdd");
    token["collector_number"] = json!("1");
    token["type_line"] = json!("Token Creature \u{2014} Goblin");
    token["oracle_id"] = json!("00000000-0000-4000-8000-0000000000aa");
    token["id"] = json!("00000000-0000-4000-8000-0000000000ab");
    all.push(token);
    serde_json::to_string(&all).unwrap()
}

/// Both schemas migrated and the fixture imported.
async fn prepare(pool: &PgPool) -> Database {
    migrate(pool).await.unwrap();
    let db = Database::from_pool(pool.clone());
    db.migrate().await.unwrap();
    db.importer()
        .import_cards(cards(fixture().as_bytes()))
        .await
        .unwrap();
    db
}

async fn serve(pool: &PgPool, source: CardSource) -> TestServer {
    TestServer::start_with_cards(pool.clone(), bot_config(), source).await
}

/// A server whose engine runs everything, over a database holding the fixture.
async fn start(pool: &PgPool) -> TestServer {
    prepare(pool).await;
    serve(
        pool,
        CardSource::new(pool.clone()).with_caps(Caps::everything()),
    )
    .await
}

fn line(zone: &str, name: &str, count: u32) -> Value {
    json!({"zone": zone, "oracle_id": oracle(name), "printing_id": null, "count": count})
}

fn body(name: &str, format: &str, lines: &[Value]) -> Value {
    json!({"name": name, "format": format, "notes": "", "cards": lines})
}

/// Sixty cards the rules corpus fully implements, legal in Modern.
fn modern() -> Value {
    body(
        "Green Burn",
        "modern",
        &[
            line("main", "Forest", 36),
            line("main", "Llanowar Elves", 4),
            line("main", "Giant Growth", 4),
            line("main", "Grizzly Bears", 4),
            line("main", "Lightning Bolt", 4),
            line("main", "Burst Lightning", 4),
            line("main", "Rampant Growth", 4),
        ],
    )
}

/// A hundred cards: Questing Beast, whose rules the corpus models in full, Auspicious Starrix, which it models in part,
/// and ninety-eight Forests.
fn commander() -> Value {
    body(
        "Beast Mode",
        "commander",
        &[
            line("commander", "Questing Beast", 1),
            line("main", "Forest", 98),
            line("main", "Auspicious Starrix", 1),
        ],
    )
}

async fn register_ann(server: &TestServer) -> Browser {
    register(server.addr, "ann", PASSWORD).await
}

async fn create(server: &TestServer, who: &Browser, deck: &Value) -> Value {
    let reply = who.post(server.addr, "/api/decks", &deck.to_string()).await;
    assert_eq!(reply.status, 201, "{}", reply.text());
    reply.json()
}

fn id_of(view: &Value) -> String {
    view["id"].as_str().unwrap().to_owned()
}

/// The deck as the database holds it, without going through the server.
async fn stored(pool: &PgPool, who: &Browser, id: &str) -> mtg_db::StoredDeck {
    let owner = Uuid::parse_str(&who.id).unwrap();
    Database::from_pool(pool.clone())
        .decks()
        .get(owner, DeckId(Uuid::parse_str(id).unwrap()))
        .await
        .unwrap()
        .unwrap()
}

fn is_stamp(text: &Value) -> bool {
    text.as_str().is_some_and(|s| {
        s.len() == 20
            && s.ends_with('Z')
            && s.bytes().enumerate().all(|(at, b)| match at {
                4 | 7 => b == b'-',
                10 => b == b'T',
                13 | 16 => b == b':',
                19 => b == b'Z',
                _ => b.is_ascii_digit(),
            })
    })
}

// ---- who may reach decks ----

#[tokio::test]
async fn a_server_without_a_database_has_no_decks() {
    let server = TestServer::start(bot_config()).await;
    let id = "4457ed35-7c10-48c8-9776-456485fdf070";
    let empty = b"" as &[u8];
    let json_headers = [("Content-Type", "application/json")];
    for (method, path) in [
        ("GET", "/api/decks".to_owned()),
        ("POST", "/api/decks".to_owned()),
        ("GET", format!("/api/decks/{id}")),
        ("PUT", format!("/api/decks/{id}")),
        ("DELETE", format!("/api/decks/{id}")),
        ("GET", format!("/api/decks/{id}/export")),
        ("POST", "/api/decks/import".to_owned()),
        ("POST", "/api/decks/check".to_owned()),
    ] {
        let reply = request(server.addr, method, &path, &json_headers, empty).await;
        assert_eq!(reply.status, 503, "{method} {path}");
        assert_eq!(reply.error_code(), "decks_unavailable", "{method} {path}");
    }
    // The built-in decks of the create-game page need no database.
    let demo = get(server.addr, "/api/demo-decks").await;
    assert_eq!(demo.status, 200);
    assert!(demo.json().as_array().unwrap().len() >= 2);
}

#[sqlx::test(migrations = false)]
async fn every_deck_route_needs_a_signed_in_account(pool: PgPool) {
    let server = start(&pool).await;
    let id = "4457ed35-7c10-48c8-9776-456485fdf070";
    let json_headers = [("Content-Type", "application/json")];
    for (method, path) in [
        ("GET", "/api/decks".to_owned()),
        ("POST", "/api/decks".to_owned()),
        ("GET", format!("/api/decks/{id}")),
        ("PUT", format!("/api/decks/{id}")),
        ("DELETE", format!("/api/decks/{id}")),
        ("GET", format!("/api/decks/{id}/export")),
        ("POST", "/api/decks/import".to_owned()),
        ("POST", "/api/decks/check".to_owned()),
    ] {
        let reply = request(server.addr, method, &path, &json_headers, b"{}").await;
        assert_eq!(reply.status, 401, "{method} {path}");
        assert_eq!(reply.error_code(), "unauthenticated", "{method} {path}");
    }
    let stale = request(
        server.addr,
        "GET",
        "/api/decks",
        &[("Cookie", &format!("mtg_session={}", "ab".repeat(32)))],
        b"",
    )
    .await;
    assert_eq!(stale.status, 401);
    assert!(
        stale
            .headers_named("set-cookie")
            .iter()
            .any(|c| c.starts_with("mtg_session=") && c.contains("Max-Age=0")),
        "a cookie the server does not honour is told to go: {:?}",
        stale.headers
    );
}

#[sqlx::test(migrations = false)]
async fn an_account_never_reaches_another_accounts_deck(pool: PgPool) {
    let server = start(&pool).await;
    let ann = register_ann(&server).await;
    let bo = register(server.addr, "bob", PASSWORD).await;
    let mine = create(&server, &ann, &modern()).await;
    let id = id_of(&mine);
    let before = stored(&pool, &ann, &id).await;

    let path = format!("/api/decks/{id}");
    for reply in [
        bo.get(server.addr, &path).await,
        bo.put(server.addr, &path, &modern().to_string()).await,
        bo.delete(server.addr, &path).await,
        bo.get(server.addr, &format!("{path}/export")).await,
        bo.post(
            server.addr,
            "/api/decks/check",
            &json!({"deck": id}).to_string(),
        )
        .await,
    ] {
        assert_eq!(reply.status, 404, "{}", reply.text());
        assert_eq!(reply.error_code(), "not_found");
    }
    let list = bo.get(server.addr, "/api/decks").await.json();
    assert_eq!(list["decks"], json!([]));
    assert_eq!(
        stored(&pool, &ann, &id).await,
        before,
        "nothing of Ann's changed"
    );
    assert_eq!(
        ann.get(server.addr, "/api/decks").await.json()["decks"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
}

#[sqlx::test(migrations = false)]
async fn a_request_from_another_site_changes_nothing(pool: PgPool) {
    let server = start(&pool).await;
    let ann = register_ann(&server).await;
    let mine = create(&server, &ann, &modern()).await;
    let id = id_of(&mine);
    let foreign = [("Origin", "https://evil.example")];
    let path = format!("/api/decks/{id}");

    for (method, target, text) in [
        ("POST", "/api/decks".to_owned(), modern().to_string()),
        ("PUT", path.clone(), commander().to_string()),
        ("DELETE", path.clone(), String::new()),
        (
            "POST",
            "/api/decks/import".to_owned(),
            json!({"text": "1 Forest"}).to_string(),
        ),
        (
            "POST",
            "/api/decks/check".to_owned(),
            json!({"deck": id}).to_string(),
        ),
    ] {
        let reply = ann
            .send(server.addr, method, &target, &text, &foreign)
            .await;
        assert_eq!(reply.status, 403, "{method} {target}");
        assert_eq!(reply.error_code(), "forbidden_origin");
    }
    let list = ann.get(server.addr, "/api/decks").await.json();
    assert_eq!(list["decks"].as_array().unwrap().len(), 1);
    assert_eq!(list["decks"][0]["name"], "Green Burn");
    // The deck's own site is no foreign site.
    let same = format!("http://{}", server.addr);
    let own = ann
        .send(
            server.addr,
            "PUT",
            &path,
            &modern().to_string(),
            &[("Origin", &same)],
        )
        .await;
    assert_eq!(own.status, 200, "{}", own.text());
}

// ---- saving, reading, replacing, deleting ----

#[sqlx::test(migrations = false)]
async fn a_saved_clean_deck_is_verified_and_reads_back_whole(pool: PgPool) {
    let server = start(&pool).await;
    let ann = register_ann(&server).await;
    let view = create(&server, &ann, &modern()).await;
    let id = id_of(&view);

    assert!(Uuid::parse_str(&id).is_ok());
    assert_eq!(
        (view["name"].as_str(), view["format"].as_str()),
        (Some("Green Burn"), Some("modern"))
    );
    assert_eq!(view["state"], "verified");
    assert_eq!(
        (view["verified"].as_bool(), view["changed"].as_bool()),
        (Some(true), Some(false))
    );
    assert!(is_stamp(&view["created_at"]) && is_stamp(&view["updated_at"]));
    assert_eq!(view["cards"].as_array().unwrap().len(), 7);
    assert_eq!(view["cards"][0]["zone"], "main");
    assert_eq!(view["cards"][0]["count"], 36);
    assert_eq!(view["cards"][0]["card"]["name"], "Forest");
    assert_eq!(view["cards"][0]["card"]["support"], "verified");
    assert_eq!(view["cards"][0]["printing_id"], Value::Null);

    let report = &view["report"];
    assert_eq!(report["legality"], json!([]));
    assert_eq!(report["summary"]["state"], "verified");
    assert_eq!(report["summary"]["counts"]["total"], 60);
    assert_eq!(report["summary"]["counts"]["verified"], 60);
    assert_eq!(report["cards"].as_array().unwrap().len(), 7);

    let check = &view["check"];
    assert_eq!(check["deck_hash"], report["deck_hash"]);
    assert_eq!(check["coverage_hash"], report["coverage_hash"]);
    assert_eq!(check["summary"], report["summary"]);
    assert!(is_stamp(&check["checked_at"]), "{}", check["checked_at"]);

    let read = ann.get(server.addr, &format!("/api/decks/{id}")).await;
    assert_eq!(read.status, 200, "{}", read.text());
    assert_eq!(read.json(), view, "what was saved is what is read");

    let list = ann.get(server.addr, "/api/decks").await.json();
    let item = &list["decks"][0];
    assert_eq!(list["decks"].as_array().unwrap().len(), 1);
    assert_eq!(item["id"], view["id"]);
    assert_eq!(
        (item["state"].as_str(), item["verified"].as_bool()),
        (Some("verified"), Some(true))
    );
    assert_eq!(item["checked_at"], check["checked_at"]);
    assert_eq!(item["summary"], report["summary"]);
}

#[sqlx::test(migrations = false)]
async fn a_deck_with_manual_cards_or_one_that_breaks_its_format_is_not_verified(pool: PgPool) {
    let server = start(&pool).await;
    let ann = register_ann(&server).await;

    // Sixty cards legal in Modern, one of which (Aven Mindcensor) the corpus models in part.
    let mut partial = modern();
    partial["name"] = json!("Beast Mode");
    partial["cards"][0]["count"] = json!(35);
    partial["cards"]
        .as_array_mut()
        .unwrap()
        .push(line("main", "Aven Mindcensor", 1));
    let manual = create(&server, &ann, &partial).await;
    assert_eq!(manual["state"], "manual_play");
    assert_eq!(
        (manual["verified"].as_bool(), manual["changed"].as_bool()),
        (Some(false), Some(false))
    );
    assert_eq!(
        manual["report"]["legality"],
        json!([]),
        "a legal Modern deck"
    );
    let counts = &manual["report"]["summary"]["counts"];
    assert_eq!(
        (
            counts["total"].as_u64(),
            counts["verified"].as_u64(),
            counts["partial"].as_u64()
        ),
        (Some(60), Some(59), Some(1))
    );
    let bird = manual["report"]["cards"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["name"] == "Aven Mindcensor")
        .unwrap();
    assert_eq!(
        (bird["zone"].as_str(), bird["support"].as_str()),
        (Some("main"), Some("partial"))
    );
    assert!(!bird["reasons"].as_array().unwrap().is_empty(), "{bird}");
    assert!(bird["reasons"][0]["span"]["end"].as_u64().unwrap() > 0);
    assert_eq!(manual["check"]["summary"]["state"], "manual_play");

    let mut short = modern();
    short["cards"][0]["count"] = json!(35);
    let blocked = create(&server, &ann, &short).await;
    assert_eq!(blocked["state"], "blocked");
    assert_eq!(blocked["verified"], false);
    assert_eq!(blocked["report"]["legality"][0]["code"], "deck_size");
    assert_eq!(blocked["check"]["summary"]["state"], "blocked");

    let list = ann.get(server.addr, "/api/decks").await.json();
    let states: Vec<(&str, &str)> = list["decks"]
        .as_array()
        .unwrap()
        .iter()
        .map(|d| (d["name"].as_str().unwrap(), d["state"].as_str().unwrap()))
        .collect();
    assert_eq!(
        states,
        [("Green Burn", "blocked"), ("Beast Mode", "manual_play")],
        "the most recently saved first"
    );
}

#[sqlx::test(migrations = false)]
async fn replacing_a_deck_runs_the_check_again_and_marks_it_at_once(pool: PgPool) {
    let server = start(&pool).await;
    let ann = register_ann(&server).await;
    let view = create(&server, &ann, &commander()).await;
    let id = id_of(&view);
    let path = format!("/api/decks/{id}");
    assert_eq!(view["state"], "manual_play");

    let mut notes = modern();
    notes["notes"] = json!("Sideboard plans:\n- more Elves");
    let replaced = ann.put(server.addr, &path, &notes.to_string()).await;
    assert_eq!(replaced.status, 200, "{}", replaced.text());
    let replaced = replaced.json();
    assert_eq!(replaced["id"], view["id"]);
    assert_eq!(replaced["created_at"], view["created_at"]);
    assert_eq!(
        (replaced["name"].as_str(), replaced["format"].as_str()),
        (Some("Green Burn"), Some("modern"))
    );
    assert_eq!(replaced["notes"], "Sideboard plans:\n- more Elves");
    assert_eq!(
        replaced["cards"].as_array().unwrap().len(),
        7,
        "the old lines are gone"
    );
    assert_eq!(
        replaced["state"], "verified",
        "a clean deck is marked at once"
    );
    assert_ne!(replaced["check"]["deck_hash"], view["check"]["deck_hash"]);

    let mut short = modern();
    short["cards"][0]["count"] = json!(34);
    let worse = ann.put(server.addr, &path, &short.to_string()).await.json();
    assert_eq!(worse["state"], "blocked");
    assert_eq!(
        worse["check"]["summary"]["state"], "blocked",
        "the stored check is the latest one"
    );
    assert_eq!(ann.get(server.addr, &path).await.json(), worse);

    assert_eq!(ann.delete(server.addr, &path).await.status, 204);
    let gone = ann.get(server.addr, &path).await;
    assert_eq!(
        (gone.status, gone.error_code().as_str()),
        (404, "not_found")
    );
    assert_eq!(ann.delete(server.addr, &path).await.status, 404);
    assert_eq!(
        ann.put(server.addr, &path, &modern().to_string())
            .await
            .status,
        404
    );
    assert_eq!(
        ann.get(server.addr, "/api/decks").await.json()["decks"],
        json!([])
    );
}

// ---- the marker ----

#[sqlx::test(migrations = false)]
async fn the_marker_follows_the_deck_the_rules_and_the_engine(pool: PgPool) {
    let server = start(&pool).await;
    let ann = register_ann(&server).await;
    let view = create(&server, &ann, &modern()).await;
    let id = id_of(&view);
    let path = format!("/api/decks/{id}");
    assert_eq!(view["verified"], true);
    let saved = stored(&pool, &ann, &id).await;

    // One edit that bypasses the check: the stored check stays, the hashes no longer match.
    let owner = Uuid::parse_str(&ann.id).unwrap();
    let repo = Database::from_pool(pool.clone()).decks();
    let mut edited = mtg_db::DeckDraft {
        name: saved.name.clone(),
        format: saved.format.clone(),
        notes: saved.notes.clone(),
        lines: saved.lines.clone(),
        check: saved.check.clone(),
    };
    edited.lines[0].count -= 1;
    edited.lines.push(DeckLine {
        zone: DeckZone::Sideboard,
        oracle_id: OracleId(oracle("Forest")),
        printing_id: None,
        count: 1,
    });
    repo.replace(owner, DeckId(Uuid::parse_str(&id).unwrap()), &edited)
        .await
        .unwrap()
        .unwrap();
    let after_edit = ann.get(server.addr, &path).await.json();
    assert_eq!(after_edit["state"], "unchecked");
    assert_eq!(
        (
            after_edit["verified"].as_bool(),
            after_edit["changed"].as_bool()
        ),
        (Some(false), Some(true))
    );
    assert_ne!(
        after_edit["check"]["deck_hash"],
        after_edit["report"]["deck_hash"]
    );
    assert_eq!(
        after_edit["check"]["coverage_hash"], after_edit["report"]["coverage_hash"],
        "the same cards"
    );
    let listed = ann.get(server.addr, "/api/decks").await.json();
    assert_eq!(listed["decks"][0]["state"], "unchecked");
    assert_eq!(listed["decks"][0]["changed"], true);

    // Saving it through the server marks it again, with the sideboard counting like any zone.
    let mut side = modern();
    side["cards"]
        .as_array_mut()
        .unwrap()
        .push(line("sideboard", "Forest", 1));
    let remarked = ann.put(server.addr, &path, &side.to_string()).await.json();
    assert_eq!(remarked["state"], "verified");
    assert_eq!(
        remarked["report"]["summary"]["zones"][1]["zone"],
        "sideboard"
    );

    // Another engine: the same deck under a server that runs nothing is unchecked, not verified.
    let weaker = serve(&pool, CardSource::new(pool.clone()).with_caps(Caps::none())).await;
    let before = stored(&pool, &ann, &id).await;
    let under_none = ann.get(weaker.addr, &path).await.json();
    assert_eq!(under_none["state"], "unchecked");
    assert_eq!(
        (
            under_none["verified"].as_bool(),
            under_none["changed"].as_bool()
        ),
        (Some(false), Some(true))
    );
    assert_eq!(
        under_none["check"]["deck_hash"],
        under_none["report"]["deck_hash"]
    );
    assert_ne!(
        under_none["check"]["coverage_hash"],
        under_none["report"]["coverage_hash"]
    );
    assert_eq!(under_none["report"]["summary"]["state"], "manual_play");
    assert_eq!(
        ann.get(weaker.addr, "/api/decks").await.json()["decks"][0]["state"],
        "unchecked"
    );
    assert_eq!(
        stored(&pool, &ann, &id).await,
        before,
        "reading a deck stores nothing"
    );

    // Other rules for a card of the deck: also unchecked.
    let mut sources = builtin::sources();
    sources.retain(|s| !s.path.ends_with("lightning_bolt.json"));
    let rules = Library::from_sources(&sources).unwrap();
    let other = serve(
        &pool,
        CardSource::new(pool.clone())
            .with_caps(Caps::everything())
            .with_rules(rules),
    )
    .await;
    let under_rules = ann.get(other.addr, &path).await.json();
    assert_eq!(
        (
            under_rules["state"].as_str(),
            under_rules["changed"].as_bool()
        ),
        (Some("unchecked"), Some(true))
    );

    // The first server still sees the deck as verified: nothing above wrote to it.
    let back = ann.get(server.addr, &path).await.json();
    assert_eq!(
        (back["state"].as_str(), back["verified"].as_bool()),
        (Some("verified"), Some(true))
    );
}

#[sqlx::test(migrations = false)]
async fn a_ban_that_leaves_the_hashes_alone_still_removes_the_marker(pool: PgPool) {
    let server = start(&pool).await;
    let ann = register_ann(&server).await;
    let view = create(&server, &ann, &modern()).await;
    let path = format!("/api/decks/{}", id_of(&view));
    assert_eq!(view["verified"], true);

    sqlx::query(
        "UPDATE cards SET legalities = jsonb_set(legalities, '{modern}', '\"banned\"') WHERE name = 'Lightning Bolt'",
    )
    .execute(&pool)
    .await
    .unwrap();
    let after = ann.get(server.addr, &path).await.json();
    assert_eq!(
        after["changed"], false,
        "neither the deck nor its cards' rules changed"
    );
    assert_eq!(after["state"], "blocked");
    assert_eq!(after["verified"], false);
    assert_eq!(after["report"]["legality"][0]["code"], "banned");
    assert_eq!(after["report"]["legality"][0]["name"], "Lightning Bolt");
    assert_eq!(
        after["check"]["summary"]["state"], "verified",
        "the stored check is the one of the save"
    );
}

// ---- the check ----

#[sqlx::test(migrations = false)]
async fn the_check_reports_a_stored_deck_or_an_unsaved_list_and_saves_nothing(pool: PgPool) {
    let server = start(&pool).await;
    let ann = register_ann(&server).await;
    let view = create(&server, &ann, &commander()).await;
    let id = id_of(&view);
    let before = stored(&pool, &ann, &id).await;

    let by_id = ann
        .post(
            server.addr,
            "/api/decks/check",
            &json!({"deck": id}).to_string(),
        )
        .await;
    assert_eq!(by_id.status, 200, "{}", by_id.text());
    assert_eq!(
        by_id.json(),
        view["report"],
        "the report of the deck as it is"
    );

    let inline = json!({
        "format": "modern",
        "cards": [line("main", "Forest", 20), line("sideboard", "Sol Ring", 1)]
    });
    let report = ann
        .post(server.addr, "/api/decks/check", &inline.to_string())
        .await
        .json();
    assert_eq!(report["format"], "modern");
    assert_eq!(report["summary"]["counts"]["total"], 21);
    let codes: Vec<&str> = report["legality"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| i["code"].as_str().unwrap())
        .collect();
    assert_eq!(codes, ["deck_size", "not_legal"]);
    assert_eq!(report["legality"][1]["name"], "Sol Ring");
    assert_eq!(report["legality"][1]["zone"], "sideboard");

    assert_eq!(stored(&pool, &ann, &id).await, before);
    let list = ann.get(server.addr, "/api/decks").await.json();
    assert_eq!(
        list["decks"].as_array().unwrap().len(),
        1,
        "nothing was saved"
    );
}

#[sqlx::test(migrations = false)]
async fn the_check_refuses_what_it_cannot_read(pool: PgPool) {
    let server = start(&pool).await;
    let ann = register_ann(&server).await;
    let cards = json!([line("main", "Forest", 1)]);
    let ghost = json!([{"zone": "main", "oracle_id": "00000000-0000-4000-8000-000000000001", "printing_id": null, "count": 1}]);
    let token = json!([{"zone": "main", "oracle_id": "00000000-0000-4000-8000-0000000000aa", "printing_id": null, "count": 1}]);
    for (what, request_body, status, code) in [
        ("nothing", json!({}), 400, "invalid_request"),
        (
            "an id and a list",
            json!({"deck": "4457ed35-7c10-48c8-9776-456485fdf070", "format": "modern", "cards": cards}),
            400,
            "invalid_request",
        ),
        (
            "a list without a format",
            json!({"cards": cards}),
            400,
            "invalid_request",
        ),
        (
            "a format without a list",
            json!({"format": "modern"}),
            400,
            "invalid_request",
        ),
        (
            "an unknown format",
            json!({"format": "brawl", "cards": cards}),
            400,
            "invalid_deck",
        ),
        (
            "a card of no database",
            json!({"format": "modern", "cards": ghost}),
            400,
            "unknown_cards",
        ),
        (
            "a token",
            json!({"format": "modern", "cards": token}),
            400,
            "unknown_cards",
        ),
        (
            "a deck id that is none",
            json!({"deck": "nope"}),
            404,
            "not_found",
        ),
        (
            "a deck nobody has",
            json!({"deck": "4457ed35-7c10-48c8-9776-456485fdf070"}),
            404,
            "not_found",
        ),
        (
            "another field",
            json!({"format": "modern", "cards": cards, "name": "x"}),
            400,
            "invalid_request",
        ),
    ] {
        let reply = ann
            .post(server.addr, "/api/decks/check", &request_body.to_string())
            .await;
        assert_eq!(
            (reply.status, reply.error_code()),
            (status, code.to_owned()),
            "{what}: {}",
            reply.text()
        );
    }
}

// ---- what a save accepts ----

#[sqlx::test(migrations = false)]
async fn a_save_is_refused_with_the_rule_it_breaks(pool: PgPool) {
    let server = start(&pool).await;
    let ann = register_ann(&server).await;
    let forest = oracle("Forest");
    let with = |change: &dyn Fn(&mut Value)| {
        let mut deck = modern();
        change(&mut deck);
        deck
    };
    let cases: Vec<(&str, Value, u16, &str, &str)> = vec![
        (
            "an empty name",
            with(&|d| d["name"] = json!("  ")),
            400,
            "invalid_deck",
            "name",
        ),
        (
            "a long name",
            with(&|d| d["name"] = json!("n".repeat(101))),
            400,
            "invalid_deck",
            "name",
        ),
        (
            "a control character in the name",
            with(&|d| d["name"] = json!("a\u{7}b")),
            400,
            "invalid_deck",
            "name",
        ),
        (
            "an unknown format",
            with(&|d| d["format"] = json!("brawl")),
            400,
            "invalid_deck",
            "format",
        ),
        (
            "long notes",
            with(&|d| d["notes"] = json!("z".repeat(10_001))),
            400,
            "invalid_deck",
            "notes",
        ),
        (
            "a zero count",
            with(&|d| d["cards"][0]["count"] = json!(0)),
            400,
            "invalid_deck",
            "line 1",
        ),
        (
            "a bad oracle id",
            with(&|d| d["cards"][1]["oracle_id"] = json!("nope")),
            400,
            "invalid_deck",
            "line 2",
        ),
        (
            "a bad zone",
            with(&|d| d["cards"][0]["zone"] = json!("graveyard")),
            400,
            "invalid_request",
            "graveyard",
        ),
        (
            "a card of no database",
            with(&|d| d["cards"][0]["oracle_id"] = json!("00000000-0000-4000-8000-000000000001")),
            400,
            "unknown_cards",
            "00000000-0000-4000-8000-000000000001",
        ),
        (
            "a token",
            with(&|d| d["cards"][0]["oracle_id"] = json!("00000000-0000-4000-8000-0000000000aa")),
            400,
            "unknown_cards",
            "0000000000aa",
        ),
        (
            "too many cards",
            with(&|d| {
                d["cards"] = json!([{"zone": "main", "oracle_id": forest, "printing_id": null, "count": 251}]);
            }),
            400,
            "invalid_deck",
            "250 cards",
        ),
        (
            "too many lines",
            with(&|d| {
                d["cards"] = json!(
                    (0..251)
                        .map(|_| line("main", "Forest", 1))
                        .collect::<Vec<_>>()
                );
            }),
            400,
            "invalid_deck",
            "250 lines",
        ),
        (
            "another field",
            with(&|d| d["owner"] = json!("me")),
            400,
            "invalid_request",
            "owner",
        ),
        (
            "no cards field",
            with(&|d| {
                d.as_object_mut().unwrap().remove("cards");
            }),
            400,
            "invalid_request",
            "cards",
        ),
        (
            "a printing of another card",
            with(&|d| d["cards"][0]["printing_id"] = json!(printing("Lightning Bolt"))),
            400,
            "invalid_deck",
            "printing",
        ),
        (
            "a printing nobody prints",
            with(&|d| d["cards"][0]["printing_id"] = json!("00000000-0000-4000-8000-000000000002")),
            400,
            "invalid_deck",
            "printing",
        ),
    ];
    for (what, deck, status, code, fragment) in cases {
        let reply = ann.post(server.addr, "/api/decks", &deck.to_string()).await;
        assert_eq!(
            (reply.status, reply.error_code()),
            (status, code.to_owned()),
            "{what}: {}",
            reply.text()
        );
        let message = reply.json()["message"].as_str().unwrap().to_owned();
        assert!(message.contains(fragment), "{what}: {message}");
    }
    assert_eq!(
        ann.get(server.addr, "/api/decks").await.json()["decks"],
        json!([]),
        "nothing was stored"
    );
    // The same rules apply to a replacement.
    let view = create(&server, &ann, &modern()).await;
    let path = format!("/api/decks/{}", id_of(&view));
    let bad = with(&|d| d["format"] = json!("brawl"));
    assert_eq!(
        ann.put(server.addr, &path, &bad.to_string()).await.status,
        400
    );
    assert_eq!(
        ann.get(server.addr, &path).await.json(),
        view,
        "a refused replacement changes nothing"
    );
}

#[sqlx::test(migrations = false)]
async fn bodies_are_json_and_bounded(pool: PgPool) {
    let server = start(&pool).await;
    let ann = register_ann(&server).await;
    let cookie = ann.cookie.as_str();
    let plain = request(
        server.addr,
        "POST",
        "/api/decks",
        &[("Cookie", cookie), ("Content-Type", "text/plain")],
        b"{}",
    )
    .await;
    assert_eq!(
        (plain.status, plain.error_code()),
        (415, "unsupported_media_type".to_owned())
    );
    let bad = ann.post(server.addr, "/api/decks", "{not json").await;
    assert_eq!(
        (bad.status, bad.error_code()),
        (400, "invalid_request".to_owned())
    );
    let huge = format!(
        r#"{{"name":"{}","format":"modern","cards":[]}}"#,
        "x".repeat(70 * 1024)
    );
    let big = ann.post(server.addr, "/api/decks", &huge).await;
    assert_eq!(
        (big.status, big.error_code()),
        (413, "payload_too_large".to_owned())
    );
    // A list for import may be bigger than a deck.
    let list = "1 Forest\n".repeat(900);
    let imported = ann
        .post(
            server.addr,
            "/api/decks/import",
            &json!({"text": list}).to_string(),
        )
        .await;
    assert_eq!(imported.status, 200, "{}", imported.text());
}

#[sqlx::test(migrations = false)]
async fn an_account_stores_a_hundred_decks_and_no_more(pool: PgPool) {
    let server = start(&pool).await;
    let ann = register_ann(&server).await;
    let bo = register(server.addr, "bob", PASSWORD).await;
    let small = body("Small", "freeform", &[line("main", "Forest", 1)]);
    let mut first = None;
    for _ in 0..100 {
        let view = create(&server, &ann, &small).await;
        first.get_or_insert(id_of(&view));
    }
    let over = ann
        .post(server.addr, "/api/decks", &small.to_string())
        .await;
    assert_eq!(
        (over.status, over.error_code()),
        (409, "deck_limit".to_owned()),
        "{}",
        over.text()
    );
    assert_eq!(
        ann.get(server.addr, "/api/decks").await.json()["decks"]
            .as_array()
            .unwrap()
            .len(),
        100
    );

    // Another account has its own hundred.
    create(&server, &bo, &small).await;
    // Replacing is no creation, and a delete frees a place.
    let path = format!("/api/decks/{}", first.unwrap());
    assert_eq!(
        ann.put(server.addr, &path, &small.to_string()).await.status,
        200
    );
    assert_eq!(ann.delete(server.addr, &path).await.status, 204);
    create(&server, &ann, &small).await;
}

#[sqlx::test(migrations = false)]
async fn a_chosen_printing_is_kept_and_must_be_the_cards_own(pool: PgPool) {
    let server = start(&pool).await;
    let ann = register_ann(&server).await;
    let mut deck = modern();
    deck["cards"][0]["printing_id"] = json!(printing("Forest").to_uppercase());
    let view = create(&server, &ann, &deck).await;
    assert_eq!(
        view["cards"][0]["printing_id"],
        printing("Forest"),
        "ids come back in one spelling"
    );
    assert_eq!(view["cards"][1]["printing_id"], Value::Null);
    let id = id_of(&view);
    assert_eq!(
        ann.get(server.addr, &format!("/api/decks/{id}"))
            .await
            .json(),
        view
    );
    // The printing is no part of what the check hashes: the same deck without it has the same marker.
    let plain = create(&server, &ann, &modern()).await;
    assert_eq!(plain["check"]["deck_hash"], view["check"]["deck_hash"]);
    assert_eq!(
        plain["check"]["coverage_hash"],
        view["check"]["coverage_hash"]
    );
}

// ---- import ----

fn zone_name_count(lines: &Value) -> Vec<(String, String, u64)> {
    lines
        .as_array()
        .unwrap()
        .iter()
        .map(|l| {
            (
                l["zone"].as_str().unwrap().to_owned(),
                l["card"]["name"].as_str().unwrap().to_owned(),
                l["count"].as_u64().unwrap(),
            )
        })
        .collect()
}

#[sqlx::test(migrations = false)]
async fn a_pasted_list_resolves_to_cards_and_reports_the_rest_line_by_line(pool: PgPool) {
    let server = start(&pool).await;
    let ann = register_ann(&server).await;
    let text = "4 Lightning Bolt\n2 lightnin bolt\n1 Fire/Ice\n3 Forest (MDD) 301\n1 Not A Real Card\nBlack Lotus\n\nSideboard\n2 Burst Lightning\n\nMaybeboard\n1 Sol Ring\n";
    let reply = ann
        .post(
            server.addr,
            "/api/decks/import",
            &json!({"text": text}).to_string(),
        )
        .await;
    assert_eq!(reply.status, 200, "{}", reply.text());
    let result = reply.json();
    assert_eq!(result["source"], "text");
    assert_eq!(
        zone_name_count(&result["cards"]),
        [
            ("main".to_owned(), "Lightning Bolt".to_owned(), 4),
            ("main".to_owned(), "Fire // Ice".to_owned(), 1),
            ("main".to_owned(), "Forest".to_owned(), 3),
            ("sideboard".to_owned(), "Burst Lightning".to_owned(), 2),
        ]
    );
    assert_eq!(
        result["cards"][2]["printing_id"],
        printing("Forest"),
        "set and number found the printing"
    );
    assert_eq!(result["cards"][0]["printing_id"], Value::Null);
    assert_eq!(
        result["cards"][0]["card"]["oracle_id"],
        oracle("Lightning Bolt")
    );
    assert_eq!(result["cards"][0]["card"]["support"], "verified");

    let unresolved = result["unresolved"].as_array().unwrap();
    assert_eq!(unresolved.len(), 2);
    assert_eq!(unresolved[0]["name"], "lightnin bolt");
    assert_eq!(
        (
            unresolved[0]["count"].as_u64(),
            unresolved[0]["zone"].as_str(),
            unresolved[0]["reason"].as_str()
        ),
        (Some(2), Some("main"), Some("unknown_card"))
    );
    let suggested: Vec<&str> = unresolved[0]["suggestions"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s["name"].as_str().unwrap())
        .collect();
    assert!(suggested.contains(&"Lightning Bolt"), "{suggested:?}");
    assert_eq!(unresolved[1]["name"], "Not A Real Card");
    assert_eq!(unresolved[1]["suggestions"], json!([]));

    assert_eq!(result["problems"].as_array().unwrap().len(), 1);
    assert_eq!(result["problems"][0]["line"], 6);
    assert_eq!(
        result["ignored"],
        json!([{"name": "Maybeboard", "count": 1}])
    );

    // Nothing was saved, and the resolved lines are the cards of a deck.
    assert_eq!(
        ann.get(server.addr, "/api/decks").await.json()["decks"],
        json!([])
    );
    let cards: Vec<Value> = result["cards"]
        .as_array()
        .unwrap()
        .iter()
        .map(|l| json!({"zone": l["zone"], "oracle_id": l["card"]["oracle_id"], "printing_id": l["printing_id"], "count": l["count"]}))
        .collect();
    let saved = ann
        .post(
            server.addr,
            "/api/decks",
            &json!({"name": "Imported", "format": "freeform", "cards": cards}).to_string(),
        )
        .await;
    assert_eq!(saved.status, 201, "{}", saved.text());
}

#[sqlx::test(migrations = false)]
async fn the_formats_the_importer_reads_are_read_here_too(pool: PgPool) {
    let server = start(&pool).await;
    let ann = register_ann(&server).await;
    let import = |text: &str, source: Option<&str>, format: Option<&str>| {
        let mut request = json!({"text": text});
        if let Some(source) = source {
            request["source"] = json!(source);
        }
        if let Some(format) = format {
            request["format"] = json!(format);
        }
        request.to_string()
    };
    let arena = "Commander\n1 Questing Beast\n\nDeck\n2 Forest\n\nSideboard\n1 Giant Growth\n";
    let reply = ann
        .post(server.addr, "/api/decks/import", &import(arena, None, None))
        .await
        .json();
    assert_eq!(reply["source"], "arena");
    assert_eq!(
        zone_name_count(&reply["cards"]),
        [
            ("commander".to_owned(), "Questing Beast".to_owned(), 1),
            ("main".to_owned(), "Forest".to_owned(), 2),
            ("sideboard".to_owned(), "Giant Growth".to_owned(), 1),
        ]
    );

    // Moxfield's first line is a commander when the deck is for Commander, and not otherwise.
    let moxfield = "1 Questing Beast (ELD) 171 *F*\n99 Forest (MDD) 301\n";
    let plain = ann
        .post(
            server.addr,
            "/api/decks/import",
            &import(moxfield, None, None),
        )
        .await
        .json();
    assert_eq!(plain["source"], "moxfield");
    assert_eq!(plain["cards"][0]["zone"], "main");
    let promoted = ann
        .post(
            server.addr,
            "/api/decks/import",
            &import(moxfield, None, Some("commander")),
        )
        .await
        .json();
    assert_eq!(promoted["cards"][0]["zone"], "commander");

    // A format named by the request wins over detection, and a bad name is a 400.
    let named = ann
        .post(
            server.addr,
            "/api/decks/import",
            &import("1 Forest\n\n1 Giant Growth\n", Some("text"), None),
        )
        .await
        .json();
    assert_eq!(named["source"], "text");
    assert_eq!(
        named["cards"][1]["zone"], "sideboard",
        "plain text: a blank line starts the sideboard"
    );
    for (text, source, format) in [
        ("1 Forest", Some("excel"), None),
        ("1 Forest", None, Some("brawl")),
    ] {
        let reply = ann
            .post(
                server.addr,
                "/api/decks/import",
                &import(text, source, format),
            )
            .await;
        assert_eq!(reply.status, 400, "{}", reply.text());
    }
    let long = ann
        .post(
            server.addr,
            "/api/decks/import",
            &import(&"x".repeat(100_001), None, None),
        )
        .await;
    assert_eq!(
        (long.status, long.error_code()),
        (400, "invalid_request".to_owned())
    );
    let many = ann
        .post(
            server.addr,
            "/api/decks/import",
            &import(&"1 Forest\n".repeat(1_001), None, None),
        )
        .await;
    assert_eq!(
        (many.status, many.error_code()),
        (400, "invalid_request".to_owned())
    );
}

// ---- export ----

/// A deck with every zone, a chosen printing and a split card.
fn shared() -> Value {
    let mut forest = line("main", "Forest", 30);
    forest["printing_id"] = json!(printing("Forest"));
    body(
        "Shared / Deck: v2!",
        "freeform",
        &[
            line("commander", "Questing Beast", 1),
            forest,
            line("main", "Fire // Ice", 2),
            line("main", "Delver of Secrets // Insectile Aberration", 1),
            line("sideboard", "Giant Growth", 2),
            line("companion", "Grizzly Bears", 1),
        ],
    )
}

fn expected(lines: &[(&str, &str, u64)]) -> Vec<(String, String, u64)> {
    let mut all: Vec<(String, String, u64)> = lines
        .iter()
        .map(|(z, n, c)| ((*z).to_owned(), (*n).to_owned(), *c))
        .collect();
    all.sort();
    all
}

#[sqlx::test(migrations = false)]
async fn every_export_reads_back_through_the_importer(pool: PgPool) {
    let server = start(&pool).await;
    let ann = register_ann(&server).await;
    let view = create(&server, &ann, &shared()).await;
    let path = format!("/api/decks/{}/export", id_of(&view));
    let all = [
        ("commander", "Questing Beast", 1),
        ("main", "Forest", 30),
        ("main", "Fire // Ice", 2),
        ("main", "Delver of Secrets // Insectile Aberration", 1),
        ("sideboard", "Giant Growth", 2),
        ("companion", "Grizzly Bears", 1),
    ];
    // MTGO has no companion zone: the card sits in the sideboard.
    let mtgo = [
        ("commander", "Questing Beast", 1),
        ("main", "Forest", 30),
        ("main", "Fire // Ice", 2),
        ("main", "Delver of Secrets // Insectile Aberration", 1),
        ("sideboard", "Giant Growth", 2),
        ("sideboard", "Grizzly Bears", 1),
    ];
    for (query, source, lines, printed) in [
        ("", "text", &all, true),
        ("?format=text", "text", &all, true),
        ("?format=arena", "arena", &all, true),
        ("?format=mtgo", "mtgo", &mtgo, false),
    ] {
        let reply = ann.get(server.addr, &format!("{path}{query}")).await;
        assert_eq!(reply.status, 200, "{query}: {}", reply.text());
        assert_eq!(
            reply.header("content-type"),
            Some("text/plain; charset=utf-8")
        );
        assert_eq!(
            reply.header("content-disposition"),
            Some("attachment; filename=\"shared-deck-v2.txt\""),
            "{query}"
        );
        let text = reply.text();
        assert_eq!(text.contains("(MDD) 301"), printed, "{query}: {text}");
        let back = ann
            .post(
                server.addr,
                "/api/decks/import",
                &json!({"text": text, "source": source}).to_string(),
            )
            .await
            .json();
        assert_eq!(back["unresolved"], json!([]), "{query}: {text}");
        assert_eq!(back["problems"], json!([]), "{query}");
        let mut got = zone_name_count(&back["cards"]);
        got.sort();
        assert_eq!(got, expected(lines), "{query}:\n{text}");
        if printed {
            let forest = back["cards"]
                .as_array()
                .unwrap()
                .iter()
                .find(|l| l["card"]["name"] == "Forest")
                .unwrap();
            assert_eq!(
                forest["printing_id"],
                printing("Forest"),
                "{query}: the printing survives"
            );
        }
    }
    // Split cards are written the way each format writes them.
    let arena = ann
        .get(server.addr, &format!("{path}?format=arena"))
        .await
        .text();
    assert!(
        arena.contains("Fire /// Ice") && arena.contains("1 Delver of Secrets\n"),
        "{arena}"
    );
    let mtgo_text = ann
        .get(server.addr, &format!("{path}?format=mtgo"))
        .await
        .text();
    assert!(
        mtgo_text.contains("2 Fire/Ice") && mtgo_text.contains("SIDEBOARD:"),
        "{mtgo_text}"
    );
}

#[sqlx::test(migrations = false)]
async fn an_export_that_would_be_misread_is_refused_and_bad_queries_are_400s(pool: PgPool) {
    let server = start(&pool).await;
    let ann = register_ann(&server).await;
    let alone = body(
        "Only a commander",
        "freeform",
        &[line("commander", "Questing Beast", 1)],
    );
    let view = create(&server, &ann, &alone).await;
    let path = format!("/api/decks/{}/export", id_of(&view));
    let mtgo = ann.get(server.addr, &format!("{path}?format=mtgo")).await;
    assert_eq!(
        (mtgo.status, mtgo.error_code()),
        (422, "export_failed".to_owned()),
        "{}",
        mtgo.text()
    );
    assert_eq!(
        ann.get(server.addr, &format!("{path}?format=arena"))
            .await
            .text(),
        "Commander\n1 Questing Beast\n"
    );
    for query in [
        "?format=dek",
        "?format=",
        "?style=arena",
        "?format=text&format=arena&x=1",
    ] {
        let reply = ann.get(server.addr, &format!("{path}{query}")).await;
        assert_eq!(
            (reply.status, reply.error_code()),
            (400, "invalid_request".to_owned()),
            "{query}"
        );
    }
    let missing = ann
        .get(
            server.addr,
            "/api/decks/4457ed35-7c10-48c8-9776-456485fdf070/export",
        )
        .await;
    assert_eq!(missing.status, 404);
    let unsigned = get(server.addr, &path).await;
    assert_eq!(unsigned.status, 401);
}
