//! Goal: a pasted list resolves by printing and by the flavor and digital names of printings through the stored index: lines of the real export corpus tie to their cards, a set and number win over a name that is no card, the index follows later imports, and an emptied printings table leaves plain names resolving as before.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::net;

use mtg_cards::Caps;
use mtg_db::Database;
use mtg_import::scryfall::{CardRecord, cards};
use mtg_server::{CardSource, migrate};
use net::{Browser, PASSWORD, TestServer, bot_config, register};
use serde_json::{Value, json};
use sqlx::PgPool;

/// The Scryfall records of the printings with a flavor or printed name that the corpus uses.
const ALIASES: &str =
    include_str!("../../../mtg-import/tests/fixtures/scryfall_alias_printings.json");

const FLAVOR_WITH_PRINTING: &str =
    include_str!("../../../mtg-import/tests/fixtures/corpus/moxfield/winota-snowball/full.txt");
const FLAVOR_NAME_ONLY: &str =
    include_str!("../../../mtg-import/tests/fixtures/corpus/moxfield/winota-snowball/mtgo.txt");
const DIGITAL_WITH_PRINTING: &str =
    include_str!("../../../mtg-import/tests/fixtures/corpus/moxfield/duel-commander/full.txt");
const DIGITAL_NAME_ONLY: &str =
    include_str!("../../../mtg-import/tests/fixtures/corpus/moxfield/duel-commander/arena.txt");
const OWNER_EXPORT: &str =
    include_str!("../../../mtg-import/tests/fixtures/real/mtgo/warrior-queen.txt");

fn records() -> Vec<CardRecord> {
    cards(ALIASES.as_bytes()).map(Result::unwrap).collect()
}

/// The printing id of the stored printing `set` `number`.
fn printing_id(set: &str, number: &str) -> String {
    records()
        .iter()
        .find(|r| r.set == set && r.collector_number == number)
        .unwrap_or_else(|| panic!("the fixture has {set} {number}"))
        .printing_id
        .to_string()
}

/// Both schemas migrated, the cards stored as an Oracle Cards file has them and the printings (with
/// their other names) stored as a Default Cards file has them.
async fn prepare(pool: &PgPool) -> Database {
    migrate(pool).await.unwrap();
    let db = Database::from_pool(pool.clone());
    db.migrate().await.unwrap();
    let mut plain = records();
    for record in &mut plain {
        record.flavor_name = None;
        record.printed_name = None;
    }
    db.importer()
        .import_cards(plain.into_iter().map(Ok))
        .await
        .unwrap();
    db.importer()
        .import_printings(records().into_iter().map(Ok))
        .await
        .unwrap();
    db
}

async fn start(pool: &PgPool) -> (TestServer, Database) {
    let db = prepare(pool).await;
    let source = CardSource::new(pool.clone()).with_caps(Caps::everything());
    let server = TestServer::start_with_cards(pool.clone(), bot_config(), source).await;
    (server, db)
}

async fn import(server: &TestServer, who: &Browser, text: &str) -> Value {
    let reply = who
        .post(
            server.addr,
            "/api/decks/import",
            &json!({"text": text}).to_string(),
        )
        .await;
    assert_eq!(reply.status, 200, "{}", reply.text());
    reply.json()
}

/// The line of an export that holds `needle`.
fn line_of(export: &str, needle: &str) -> String {
    export
        .lines()
        .find(|line| line.contains(needle))
        .unwrap_or_else(|| panic!("the export has a line with {needle:?}"))
        .to_owned()
}

/// The card name and printing of the one resolved line of an import, or the reason it failed.
fn outcome(result: &Value) -> (String, Value) {
    let cards = result["cards"].as_array().unwrap();
    assert_eq!(cards.len(), 1, "{result}");
    (
        cards[0]["card"]["name"].as_str().unwrap().to_owned(),
        cards[0]["printing_id"].clone(),
    )
}

#[sqlx::test(migrations = false)]
async fn lines_of_the_export_corpus_resolve_by_flavor_name_digital_name_and_printing(pool: PgPool) {
    let (server, _) = start(&pool).await;
    let ann = register(server.addr, "ann", PASSWORD).await;
    let cases = [
        (
            line_of(FLAVOR_WITH_PRINTING, "Plankton"),
            "Skrelv, Defector Mite",
            json!(printing_id("sld", "1926")),
        ),
        (
            line_of(FLAVOR_NAME_ONLY, "Plankton"),
            "Skrelv, Defector Mite",
            Value::Null,
        ),
        (
            line_of(DIGITAL_WITH_PRINTING, "Skittering Kitten"),
            "Masked Meower",
            json!(printing_id("om1", "96")),
        ),
        (
            line_of(DIGITAL_NAME_ONLY, "Skittering Kitten"),
            "Masked Meower",
            Value::Null,
        ),
        (
            line_of(OWNER_EXPORT, "The Ball"),
            "Fellwar Stone",
            Value::Null,
        ),
        (
            "1 A Name Nobody Prints (SLD) 1040".to_owned(),
            "Fellwar Stone",
            json!(printing_id("sld", "1040")),
        ),
    ];

    for (text, card, printing) in cases {
        let result = import(&server, &ann, &text).await;
        assert_eq!(result["unresolved"], json!([]), "{text}");
        assert_eq!(outcome(&result), (card.to_owned(), printing), "{text}");
    }
}

#[sqlx::test(migrations = false)]
async fn the_index_follows_the_stored_printings_and_plain_names_survive_an_empty_table(
    pool: PgPool,
) {
    let (server, db) = start(&pool).await;
    let ann = register(server.addr, "ann", PASSWORD).await;
    let list = "1 The Ball\n1 Fellwar Stone\n";

    let before = import(&server, &ann, list).await;
    assert_eq!(before["unresolved"], json!([]));
    assert_eq!(before["cards"].as_array().unwrap().len(), 2);

    sqlx::query("DELETE FROM printings")
        .execute(&pool)
        .await
        .unwrap();
    let emptied = import(&server, &ann, list).await;
    let names: Vec<&str> = emptied["cards"]
        .as_array()
        .unwrap()
        .iter()
        .map(|line| line["card"]["name"].as_str().unwrap())
        .collect();
    assert_eq!(names, ["Fellwar Stone"], "the plain name still resolves");
    assert_eq!(emptied["unresolved"][0]["name"], "The Ball");
    assert_eq!(emptied["unresolved"][0]["reason"], "unknown_card");

    db.importer()
        .import_printings(records().into_iter().map(Ok))
        .await
        .unwrap();
    let restored = import(&server, &ann, list).await;
    assert_eq!(restored["unresolved"], json!([]));
    assert_eq!(restored["cards"].as_array().unwrap().len(), 2);
}
