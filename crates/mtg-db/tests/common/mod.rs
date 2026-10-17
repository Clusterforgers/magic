//! Goal: shared helpers that push mtg-import's recorded fixture files through the real parsers into a test database.
// Every test binary uses a different subset of these helpers.
#![allow(dead_code)]

pub mod precon;
pub mod search;

use std::fs::File;
use std::path::{Path, PathBuf};

use mtg_core::color::{Color, ColorSet};
use mtg_db::model::Card;
use mtg_db::{CardRepo, Database};
use mtg_import::ids::{OracleId, PrintingId};
use mtg_import::mtgjson::{PreconDeck, parse_deck};
use mtg_import::resolve::{CardIndex, normalize_name};
use mtg_import::scryfall::{CardRecord, RulingRecord, ScryfallError, cards, rulings};
use serde_json::Value;
use sqlx::PgPool;
use uuid::Uuid;

/// Path of a file in mtg-import's recorded fixtures.
pub fn fixture_path(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../mtg-import/tests/fixtures")
        .join(name)
}

/// The recorded Scryfall card file, streamed from disk.
pub fn card_stream() -> impl Iterator<Item = Result<CardRecord, ScryfallError>> + Send {
    let file = File::open(fixture_path("scryfall_oracle_cards.json"))
        .unwrap_or_else(|e| panic!("card fixture opens: {e}"));
    cards(file)
}

/// The recorded Scryfall rulings file, streamed from disk.
pub fn ruling_stream() -> impl Iterator<Item = Result<RulingRecord, ScryfallError>> + Send {
    let file = File::open(fixture_path("scryfall_rulings.json"))
        .unwrap_or_else(|e| panic!("ruling fixture opens: {e}"));
    rulings(file)
}

/// Every usable record of the card fixture.
pub fn fixture_records() -> Vec<CardRecord> {
    card_stream().filter_map(Result::ok).collect()
}

/// The fixture record named `name`.
pub fn record(name: &str) -> CardRecord {
    fixture_records()
        .into_iter()
        .find(|card| card.name == name)
        .unwrap_or_else(|| panic!("fixture has no card {name:?}"))
}

/// The recorded MTGJSON Commander deck.
pub fn precon_deck() -> PreconDeck {
    let file = File::open(fixture_path("mtgjson_deck_commander.json"))
        .unwrap_or_else(|e| panic!("deck fixture opens: {e}"));
    parse_deck(file).unwrap_or_else(|e| panic!("deck fixture parses: {e}"))
}

/// A database over the per-test pool, whose schema `sqlx::test` already migrated.
pub fn db(pool: PgPool) -> Database {
    Database::from_pool(pool)
}

/// A database holding the card fixture.
pub async fn seeded(pool: PgPool) -> Database {
    let db = db(pool);
    db.importer()
        .import_cards(card_stream())
        .await
        .unwrap_or_else(|e| panic!("fixture cards import: {e}"));
    db
}

/// A color set from WUBRG letters.
pub fn colors(symbols: &str) -> ColorSet {
    symbols.chars().filter_map(Color::from_symbol).collect()
}

/// Oracle id of a fixture card.
pub fn oracle_of(name: &str) -> OracleId {
    record(name).oracle_id
}

/// Names of cards, in the given order.
pub fn names(cards: &[Card]) -> Vec<String> {
    cards.iter().map(|card| card.name.clone()).collect()
}

/// `names` sorted the way the database sorts them: byte order of the normalized name.
pub fn in_search_order(names: &[&str]) -> Vec<String> {
    let mut sorted: Vec<String> = names.iter().map(|&name| name.to_owned()).collect();
    sorted.sort_by_key(|name| normalize_name(name));
    sorted
}

/// Scryfall-style JSON for the `n`th synthetic card (ids derive from `n`).
pub fn synthetic_card_json(n: u128) -> String {
    format!(
        r#"{{"id":"{}","oracle_id":"{}","name":"Synthetic {n}","cmc":1.0,"type_line":"Instant","colors":["R"],"color_identity":["R"]}}"#,
        Uuid::from_u128(n + 1_000_000),
        Uuid::from_u128(n),
    )
}

/// Oracle id of the `n`th synthetic card.
pub fn synthetic_oracle(n: u128) -> OracleId {
    OracleId(Uuid::from_u128(n).to_string())
}

/// A copy of `base` standing for another card: new oracle id, printing id and name.
pub fn another_card(base: &CardRecord, n: u128, name: &str) -> CardRecord {
    let mut card = base.clone();
    card.oracle_id = OracleId(Uuid::from_u128(n).to_string());
    card.printing_id = PrintingId(Uuid::from_u128(n + 1_000_000).to_string());
    name.clone_into(&mut card.name);
    name.clone_into(&mut card.front.name);
    card
}

/// A copy of `base` standing for another printing of the same card.
pub fn another_printing(
    base: &CardRecord,
    n: u128,
    set: &str,
    released: Option<&str>,
) -> CardRecord {
    let mut card = base.clone();
    card.printing_id = PrintingId(Uuid::from_u128(n + 2_000_000).to_string());
    set.clone_into(&mut card.set);
    card.collector_number = n.to_string();
    card.released_at = released.map(str::to_owned);
    card
}

/// Rows per table: `(cards, printings, rulings)`.
pub async fn row_counts(pool: &PgPool) -> (i64, i64, i64) {
    let count = |table: &'static str| async move {
        sqlx::query_scalar::<_, i64>(sqlx::AssertSqlSafe(format!("SELECT count(*) FROM {table}")))
            .fetch_one(pool)
            .await
            .unwrap_or_else(|e| panic!("count query runs: {e}"))
    };
    (
        count("cards").await,
        count("printings").await,
        count("rulings").await,
    )
}

/// Every row of `table` as JSON ordered by `order`, optionally without timestamp columns.
pub async fn snapshot(pool: &PgPool, table: &str, order: &str, with_timestamps: bool) -> Value {
    let strip = if with_timestamps {
        ""
    } else {
        " - 'created_at' - 'updated_at'"
    };
    let sql = format!(
        "SELECT coalesce(jsonb_agg(to_jsonb(t){strip} ORDER BY {order}), '[]'::jsonb) FROM {table} t"
    );
    sqlx::query_scalar::<_, Value>(sqlx::AssertSqlSafe(sql))
        .fetch_one(pool)
        .await
        .unwrap_or_else(|e| panic!("snapshot query runs: {e}"))
}

/// Empties the card tables (and everything that references them) between cases of one test.
pub async fn reset(pool: &PgPool) {
    sqlx::raw_sql("TRUNCATE cards CASCADE")
        .execute(pool)
        .await
        .unwrap_or_else(|e| panic!("truncate failed: {e}"));
}

/// The name index built from the database and the one built from the fixture records in memory.
pub async fn card_index_pair(database: &Database) -> (CardIndex, CardIndex) {
    let stored = database
        .cards()
        .card_index()
        .await
        .unwrap_or_else(|e| panic!("index loads: {e}"));
    (stored, CardIndex::from_records(&fixture_records()))
}

/// A card repository over a pool, for tests that read only.
pub fn repo(pool: &PgPool) -> CardRepo {
    Database::from_pool(pool.clone()).cards()
}
