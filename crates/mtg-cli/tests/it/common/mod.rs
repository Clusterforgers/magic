//! Goal: shared helpers for the integration tests: a scripted HTTP server on a local port that logs every request it sees, fixture files and a cache seeded from them.
// Every test binary uses a different subset of these helpers.
#![allow(dead_code)]

pub mod server;

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Duration;

use flate2::Compression;
use flate2::write::GzEncoder;
use mtg_cli::cache::{BulkRecord, Cache, DeckRecord, FileRecord};
use mtg_cli::files::sha256_hex;
use mtg_cli::http::{Http, HttpConfig, Validators};
use mtg_cli::progress::{Captured, Progress};
use serde_json::Value;
use sqlx::PgPool;

/// A path in mtg-import's recorded fixtures.
pub fn fixture_path(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../mtg-import/tests/fixtures")
        .join(name)
}

/// The bytes of a recorded fixture.
pub fn fixture(name: &str) -> Vec<u8> {
    fs::read(fixture_path(name)).unwrap_or_else(|e| panic!("fixture {name} reads: {e}"))
}

/// Gzip of some bytes.
pub fn gzip(bytes: &[u8]) -> Vec<u8> {
    let mut encoder = GzEncoder::new(Vec::new(), Compression::fast());
    encoder
        .write_all(bytes)
        .unwrap_or_else(|e| panic!("gzip writes: {e}"));
    encoder
        .finish()
        .unwrap_or_else(|e| panic!("gzip finishes: {e}"))
}

/// Pseudo-random bytes that do not compress, so a download has a size worth cutting.
pub fn noise(length: usize, seed: u64) -> Vec<u8> {
    let mut state = seed | 1;
    (0..length)
        .map(|_| {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            state.to_le_bytes()[0]
        })
        .collect()
}

/// A gzip file that is whole and about `length` bytes long.
pub fn gzip_file_of(length: usize, seed: u64) -> Vec<u8> {
    gzip(&noise(length, seed))
}

/// An HTTP client with the real behaviour but no timers of its own and a short, exact gap.
pub fn client(progress: &Progress, gap: Duration, attempts: u32, backoff: Duration) -> Http {
    let config = HttpConfig {
        user_agent: "mtg-cli-test/1.0".into(),
        min_gap: gap,
        connect_timeout: None,
        read_timeout: None,
        attempts,
        backoff,
    };
    Http::new(&config, progress.clone()).unwrap_or_else(|e| panic!("client builds: {e}"))
}

/// A progress sink that keeps what it is told.
pub fn progress() -> (Progress, Captured) {
    Progress::buffer()
}

/// Writes a Scryfall bulk file into the cache and records it in the manifest.
pub fn put_scryfall(cache: &Cache, kind: &str, file_name: &str, bytes: &[u8]) {
    cache
        .prepare("scryfall")
        .unwrap_or_else(|e| panic!("cache directory: {e}"));
    let relative = format!("scryfall/{file_name}");
    fs::write(cache.path(&relative), bytes).unwrap_or_else(|e| panic!("cache file: {e}"));
    let mut manifest = cache
        .manifest()
        .unwrap_or_else(|e| panic!("manifest reads: {e}"));
    manifest.scryfall.insert(
        kind.to_owned(),
        BulkRecord {
            file: record(&relative, bytes),
            updated_at: "2026-10-18T21:02:00.000+00:00".into(),
        },
    );
    cache
        .save(&manifest)
        .unwrap_or_else(|e| panic!("manifest saves: {e}"));
}

/// The oracle fixture with these ranks (by card name) in place of its own, as the cached Oracle Cards file.
pub fn put_ranks(cache: &Cache, ranks: &[(&str, Option<u32>)]) {
    let mut cards: Value = serde_json::from_slice(&fixture("scryfall_oracle_cards.json"))
        .unwrap_or_else(|e| panic!("the oracle fixture reads: {e}"));
    for card in cards.as_array_mut().into_iter().flatten() {
        let rank = ranks
            .iter()
            .find(|(name, _)| card["name"] == *name)
            .and_then(|(_, rank)| *rank);
        card["edhrec_rank"] = rank.map_or(Value::Null, Value::from);
    }
    let bytes = serde_json::to_vec(&cards).unwrap_or_else(|e| panic!("the cards write: {e}"));
    put_scryfall(
        cache,
        "oracle_cards",
        "oracle-cards-ranked.json.gz",
        &gzip(&bytes),
    );
}

/// Writes an MTGJSON deck file into the cache and records it in the manifest.
pub fn put_deck(cache: &Cache, name: &str, bytes: &[u8]) {
    cache
        .prepare("mtgjson/decks")
        .unwrap_or_else(|e| panic!("cache directory: {e}"));
    let relative = format!("mtgjson/decks/{name}.json");
    fs::write(cache.path(&relative), bytes).unwrap_or_else(|e| panic!("cache file: {e}"));
    let mut manifest = cache
        .manifest()
        .unwrap_or_else(|e| panic!("manifest reads: {e}"));
    manifest.mtgjson.decks.insert(
        name.to_owned(),
        DeckRecord {
            file: record(&relative, bytes),
            build: None,
            validators: Validators::default(),
        },
    );
    cache
        .save(&manifest)
        .unwrap_or_else(|e| panic!("manifest saves: {e}"));
}

fn record(relative: &str, bytes: &[u8]) -> FileRecord {
    FileRecord {
        file: relative.to_owned(),
        url: format!("http://fixture.test/{relative}"),
        size: u64::try_from(bytes.len()).unwrap_or(u64::MAX),
        sha256: sha256_hex(bytes),
        fetched_at: "2026-10-19T08:00:00Z".into(),
    }
}

/// A cache holding the repo's Scryfall card and ruling fixtures and its MTGJSON deck fixture.
pub fn fixture_cache(dir: &Path) -> Cache {
    let cache = Cache::new(dir);
    put_scryfall(
        &cache,
        "oracle_cards",
        "oracle-cards-fixture.json.gz",
        &gzip(&fixture("scryfall_oracle_cards.json")),
    );
    put_scryfall(
        &cache,
        "rulings",
        "rulings-fixture.json",
        &fixture("scryfall_rulings.json"),
    );
    put_deck(
        &cache,
        "BreedLethality_C16",
        &fixture("mtgjson_deck_commander.json"),
    );
    cache
}

/// Every row of `table` as JSON ordered by `order`, timestamps included.
pub async fn snapshot(pool: &PgPool, table: &str, order: &str) -> Value {
    let sql = format!(
        "SELECT coalesce(jsonb_agg(to_jsonb(t) ORDER BY {order}), '[]'::jsonb) FROM {table} t"
    );
    sqlx::query_scalar::<_, Value>(sqlx::AssertSqlSafe(sql))
        .fetch_one(pool)
        .await
        .unwrap_or_else(|e| panic!("snapshot of {table} reads: {e}"))
}

/// The card data tables with the column that orders them, for comparing the database before and after.
pub const DATA_TABLES: [(&str, &str); 5] = [
    ("cards", "oracle_id"),
    ("printings", "printing_id"),
    ("rulings", "id"),
    ("precon_decks", "id"),
    ("precon_deck_cards", "deck_id, position"),
];

/// A snapshot of every card data table.
pub async fn data(pool: &PgPool) -> Vec<Value> {
    let mut tables = Vec::new();
    for (table, order) in DATA_TABLES {
        tables.push(snapshot(pool, table, order).await);
    }
    tables
}

/// The number of rows in `table`.
pub async fn count(pool: &PgPool, table: &str) -> i64 {
    sqlx::query_scalar::<_, i64>(sqlx::AssertSqlSafe(format!("SELECT count(*) FROM {table}")))
        .fetch_one(pool)
        .await
        .unwrap_or_else(|e| panic!("count of {table} reads: {e}"))
}
