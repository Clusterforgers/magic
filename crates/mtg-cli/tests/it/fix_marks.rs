//! Goal: `fix-marks` ranks the marked cards by the accounts that marked them, joins each card's support state of the rules in force (a card with a rules file is verified, one without is manual), shows the newest note, honours `--limit`, and says so when no card is marked.

#![allow(clippy::unwrap_used)]

use crate::common;

use mtg_cli::args::FixMarksArgs;
use mtg_cli::cache::Cache;
use mtg_cli::fix_marks::fix_marks;
use mtg_cli::ingest::{IngestOptions, ingest};
use mtg_db::Database;
use mtg_import::ids::OracleId;
use sqlx::PgPool;
use sqlx::types::uuid::Uuid;

use common::{fixture_cache, progress};

async fn seeded(pool: PgPool, dir: &std::path::Path) -> Database {
    let cache: Cache = fixture_cache(dir);
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

/// Marks the card `name` for `accounts` accounts, the first with `note`.
async fn mark(db: &Database, name: &str, accounts: u128, note: Option<&str>) {
    let oracle: String = sqlx::query_scalar("SELECT oracle_id::text FROM cards WHERE name = $1")
        .bind(name)
        .fetch_one(db.pool())
        .await
        .unwrap();
    for account in 1..=accounts {
        let note = if account == accounts { note } else { None };
        db.fix_marks()
            .mark(Uuid::from_u128(account), &OracleId(oracle.clone()), note)
            .await
            .unwrap()
            .unwrap();
    }
}

fn args(limit: u32) -> FixMarksArgs {
    FixMarksArgs { limit }
}

#[sqlx::test]
async fn the_table_ranks_by_marks_and_shows_the_support_state_and_the_newest_note(pool: PgPool) {
    let dir = tempfile::tempdir().unwrap();
    let db = seeded(pool, dir.path()).await;
    mark(&db, "Lightning Bolt", 2, Some("deals the wrong damage")).await;
    mark(&db, "Jace, the Mind Sculptor", 3, Some("never\nresolves")).await;
    mark(&db, "Sol Ring", 1, None).await;

    let text = fix_marks(&db, &args(50)).await.unwrap();

    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(
        lines[0],
        "fix marks (top 3 of 3 marked cards; marks: accounts that marked the card)"
    );
    assert!(
        lines[1].contains("marks") && lines[1].contains("support"),
        "{text}"
    );
    assert!(
        lines[2].contains("  3  ") && lines[2].contains("Jace, the Mind Sculptor"),
        "{text}"
    );
    assert!(lines[2].ends_with("never resolves"), "{text}");
    assert!(
        lines[3].contains("Lightning Bolt") && lines[3].ends_with("deals the wrong damage"),
        "{text}"
    );
    assert!(
        lines[4].contains("Sol Ring") && lines[4].contains("  1  "),
        "{text}"
    );
    for line in &lines[2..] {
        let word = ["verified", "partial", "manual", "stale", "unplayable"]
            .iter()
            .any(|word| line.contains(&format!("  {word}")));
        assert!(word, "every row names a support state: {line}");
    }
}

#[sqlx::test]
async fn the_limit_cuts_the_table_but_not_the_count_in_its_header(pool: PgPool) {
    let dir = tempfile::tempdir().unwrap();
    let db = seeded(pool, dir.path()).await;
    mark(&db, "Lightning Bolt", 2, None).await;
    mark(&db, "Sol Ring", 1, None).await;

    let text = fix_marks(&db, &args(1)).await.unwrap();

    assert!(
        text.starts_with("fix marks (top 1 of 2 marked cards;"),
        "{text}"
    );
    assert_eq!(text.lines().count(), 3, "{text}");
    assert!(!text.contains("Sol Ring"), "{text}");
}

#[sqlx::test]
async fn with_no_marks_the_command_says_so(pool: PgPool) {
    let dir = tempfile::tempdir().unwrap();
    let db = seeded(pool, dir.path()).await;

    let text = fix_marks(&db, &args(50)).await.unwrap();

    assert_eq!(text, "no card is marked as important to fix yet");
}

#[sqlx::test(migrations = false)]
async fn a_database_without_the_schema_asks_for_ingest(pool: PgPool) {
    let db = Database::from_pool(pool);

    let error = fix_marks(&db, &args(50)).await.unwrap_err();

    assert!(error.to_string().contains("mtg-cli ingest"), "{error}");
}
