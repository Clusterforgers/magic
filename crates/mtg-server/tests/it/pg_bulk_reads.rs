//! Goal: the bulk read endpoints work end to end on Postgres: only a signed-in account reaches them, the list holds a pending `differs` draft whose clauses differ only in wording and not one that drops a clause, an approval records one `correct` per listed card on its current hash, a stale hash or a card that is no candidate gets nothing, and an approved card leaves the list.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::net;

use mtg_cards::builtin;
use mtg_cards::card::Card;
use mtg_cards::support::read_hash;
use mtg_db::{Database, DraftKey, NewDraft, SOURCE_FORGE};
use mtg_import::ids::OracleId;
use mtg_import::scryfall::cards;
use mtg_server::{CardSource, migrate, printed_card};
use net::{Browser, PASSWORD, TestServer, bot_config, register, request};
use serde_json::{Value, json};
use sqlx::PgPool;

/// A Forge draft whose one differing clause says "discard X cards, where X is the number of cards in your hand".
const WORDING: (&str, &str) = (
    include_str!("../../../mtg-cli/drafts/slate_of_ancestry.json"),
    include_str!("../../../mtg-cli/drafts/fixtures/slate_of_ancestry.json"),
);

/// A Forge draft that leaves out a clause ("can block creatures with shadow"), so it is no candidate.
const DROPS: (&str, &str) = (
    include_str!("../../../mtg-cli/drafts/aether_web.json"),
    include_str!("../../../mtg-cli/drafts/fixtures/aether_web.json"),
);

/// Imports the card records and stores each draft as its card's pending draft marked `differs`; returns their keys.
async fn seeded(pool: &PgPool) -> (Database, DraftKey, DraftKey) {
    migrate(pool).await.unwrap();
    let db = Database::from_pool(pool.clone());
    db.migrate().await.unwrap();
    let records = format!("[{},{}]", WORDING.1, DROPS.1);
    db.importer()
        .import_cards(cards(records.as_bytes()))
        .await
        .unwrap();
    let mut keys = Vec::new();
    for (rules, _) in [WORDING, DROPS] {
        let card: Card = serde_json::from_str(rules).unwrap();
        let id = OracleId(card.oracle_id.clone());
        let stored = db.cards().get(&id).await.unwrap().unwrap();
        let key = DraftKey {
            oracle_id: id,
            read_hash: read_hash(&printed_card(&stored), &card),
        };
        db.drafts()
            .insert(&[NewDraft {
                oracle_id: key.oracle_id.clone(),
                read_hash: key.read_hash.clone(),
                rules: serde_json::to_value(&card).unwrap(),
                source: SOURCE_FORGE.to_owned(),
                oracle_matches: true,
                differs: true,
                proof: None,
            }])
            .await
            .unwrap();
        keys.push(key);
    }
    let drops = keys.pop().unwrap();
    let wording = keys.pop().unwrap();
    (db, wording, drops)
}

async fn start(pool: &PgPool) -> (DraftKey, DraftKey, TestServer) {
    let (_, wording, drops) = seeded(pool).await;
    let source = CardSource::new(pool.clone()).with_rules(builtin::library().unwrap().clone());
    let server = TestServer::start_with_cards(pool.clone(), bot_config(), source).await;
    (wording, drops, server)
}

async fn listed(server: &TestServer, who: &Browser) -> Value {
    let reply = who.get(server.addr, "/api/reads/bulk").await;
    assert_eq!(reply.status, 200, "{}", reply.text());
    reply.json()
}

async fn approve(server: &TestServer, who: &Browser, keys: &[(&str, &str)]) -> net::Reply {
    let cards: Vec<Value> = keys
        .iter()
        .map(|(id, hash)| json!({"oracle_id": id, "read_hash": hash}))
        .collect();
    who.put(
        server.addr,
        "/api/reads/bulk",
        &json!({ "cards": cards }).to_string(),
    )
    .await
}

#[sqlx::test(migrations = false)]
async fn the_bulk_routes_need_a_signed_in_account(pool: PgPool) {
    let (_, _, server) = start(&pool).await;
    let json_headers = [("Content-Type", "application/json")];
    for method in ["GET", "PUT"] {
        let reply = request(server.addr, method, "/api/reads/bulk", &json_headers, b"{}").await;
        assert_eq!(reply.status, 401, "{method}");
        assert_eq!(reply.error_code(), "unauthenticated", "{method}");
    }
    let ann = register(server.addr, "ann", PASSWORD).await;
    let empty = approve(&server, &ann, &[]).await;
    assert_eq!(empty.status, 400, "{}", empty.text());
}

#[sqlx::test(migrations = false)]
async fn the_list_holds_a_wording_only_draft_and_not_one_that_drops_a_clause(pool: PgPool) {
    let (wording, _, server) = start(&pool).await;
    let ann = register(server.addr, "ann", PASSWORD).await;
    let list = listed(&server, &ann).await;
    let cards = list["cards"].as_array().unwrap();
    assert_eq!(cards.len(), 1, "{list}");
    let card = &cards[0];
    assert_eq!(card["name"], "Slate of Ancestry");
    assert_eq!(card["oracle_id"], wording.oracle_id.as_str());
    assert_eq!(card["read_hash"], wording.read_hash);
    assert_eq!(card["rule"], "discard_hand");
    let clause = &card["clauses"][0];
    assert!(
        clause["printed"]
            .as_str()
            .unwrap()
            .contains("Discard your hand")
    );
    assert!(
        clause["engine"]
            .as_str()
            .unwrap()
            .contains("Discard X cards")
    );
    assert_eq!(clause["rules"], json!(["discard_hand"]));
    assert_eq!(list["rules"][0]["name"], "discard_hand");
    assert_ne!(list["rules"][0]["summary"].as_str().unwrap(), "");
}

#[sqlx::test(migrations = false)]
async fn an_approval_records_one_correct_per_listed_card_and_nothing_else(pool: PgPool) {
    let (wording, drops, server) = start(&pool).await;
    let ann = register(server.addr, "ann", PASSWORD).await;
    let bob = register(server.addr, "bob", PASSWORD).await;

    let stale = approve(
        &server,
        &ann,
        &[(wording.oracle_id.as_str(), &"0".repeat(32))],
    )
    .await;
    assert_eq!(stale.status, 200, "{}", stale.text());
    assert_eq!(
        stale.json(),
        json!({"approved": [], "stale": [wording.oracle_id.as_str()]})
    );
    let card = ann
        .get(
            server.addr,
            &format!("/api/reads/{}", wording.oracle_id.as_str()),
        )
        .await
        .json();
    assert_eq!(card["mine"], Value::Null, "a stale hash records nothing");

    let reply = approve(
        &server,
        &ann,
        &[
            (wording.oracle_id.as_str(), &wording.read_hash),
            (wording.oracle_id.as_str(), &wording.read_hash),
            (drops.oracle_id.as_str(), &drops.read_hash),
        ],
    )
    .await;
    assert_eq!(reply.status, 200, "{}", reply.text());
    assert_eq!(
        reply.json(),
        json!({"approved": [wording.oracle_id.as_str()], "stale": [drops.oracle_id.as_str()]})
    );

    let card = ann
        .get(
            server.addr,
            &format!("/api/reads/{}", wording.oracle_id.as_str()),
        )
        .await
        .json();
    assert_eq!(card["state"], "approved");
    assert_eq!(
        (card["correct"].as_u64(), card["wrong"].as_u64()),
        (Some(1), Some(0))
    );
    assert_eq!(card["mine"]["verdict"], "correct");
    let other = ann
        .get(
            server.addr,
            &format!("/api/reads/{}", drops.oracle_id.as_str()),
        )
        .await
        .json();
    assert_eq!(
        other["state"], "pending",
        "a card that is no candidate gets no verdict"
    );
    assert_eq!(other["mine"], Value::Null);

    for who in [&ann, &bob] {
        let list = listed(&server, who).await;
        assert_eq!(list["cards"], json!([]), "an approved card leaves the list");
    }
}
