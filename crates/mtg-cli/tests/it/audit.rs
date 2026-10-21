//! Goal: `audit` lists a seeded sample of the stored cards the compiler reads, each with its oracle text, its reading and its JSON; the same seed lists the same cards, the count bounds the list, and a database without cards or without a schema says how to fill it.

use crate::common;

use mtg_cli::args::AuditArgs;
use mtg_cli::audit::audit;
use mtg_cli::cache::Cache;
use mtg_cli::error::CliError;
use mtg_cli::ingest::{IngestOptions, ingest};
use mtg_db::Database;
use sqlx::PgPool;

use common::{fixture_cache, progress};

async fn loaded(pool: PgPool, cache_dir: &std::path::Path) -> Database {
    let cache: Cache = fixture_cache(cache_dir);
    let db = Database::from_pool(pool);
    let (progress, _) = progress();
    let options = IngestOptions {
        force: false,
        batch_size: 500,
    };
    ingest(&cache, &db, &options, &progress)
        .await
        .unwrap_or_else(|e| panic!("ingest runs: {e}"));
    db
}

fn args(count: usize, seed: u64) -> AuditArgs {
    AuditArgs {
        count,
        seed,
        file: None,
    }
}

#[sqlx::test]
async fn the_listing_shows_oracle_text_reading_and_json_of_compiled_cards(pool: PgPool) {
    let dir = tempfile::tempdir().unwrap();
    let db = loaded(pool, dir.path()).await;

    let text = audit(&db, &args(100, 1)).await.unwrap();

    assert!(text.starts_with("audit: seed 1, "), "{text}");
    for wanted in [
        "## Lightning Bolt [Full 1/1] ",
        "  oracle  Lightning Bolt deals 3 damage to any target.",
        "  reads   ~ deals 3 damage to any target.",
        "  json    {\"Spell\":",
    ] {
        assert!(text.contains(wanted), "{wanted} missing from\n{text}");
    }
}

#[sqlx::test]
async fn the_same_seed_lists_the_same_cards_and_the_count_bounds_the_list(pool: PgPool) {
    let dir = tempfile::tempdir().unwrap();
    let db = loaded(pool, dir.path()).await;

    let all = audit(&db, &args(100, 3)).await.unwrap();
    assert_eq!(all, audit(&db, &args(100, 3)).await.unwrap());
    let one = audit(&db, &args(1, 3)).await.unwrap();

    assert!(one.starts_with("audit: seed 3, 1 cards\n"), "{one}");
    assert_eq!(one.matches("\n## ").count(), 1, "{one}");
    assert!(all.matches("\n## ").count() > 1, "{all}");
}

#[sqlx::test]
async fn a_database_without_cards_says_how_to_load_them(pool: PgPool) {
    let error = audit(&Database::from_pool(pool), &args(10, 1))
        .await
        .unwrap_err();

    assert!(matches!(error, CliError::Config(_)), "{error}");
    assert!(error.to_string().contains("mtg-cli ingest"), "{error}");
}

#[sqlx::test(migrations = false)]
async fn a_database_without_a_schema_says_how_to_apply_it(pool: PgPool) {
    let error = audit(&Database::from_pool(pool), &args(10, 1))
        .await
        .unwrap_err();

    assert!(matches!(error, CliError::Config(_)), "{error}");
    assert!(
        error.to_string().contains("schema is not applied"),
        "{error}"
    );
}
