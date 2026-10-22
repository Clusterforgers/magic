//! Goal: the commands as a person meets them: a dry run needs no database while an ingest does, the status report shows the cache, the rows and the latest runs of a loaded database and says so for an empty one, and every summary is one line.

use crate::common;

use std::time::Duration;

use mtg_cli::args::{Cli, Command, IngestArgs};
use mtg_cli::cache::Cache;
use mtg_cli::fetch::decks::DeckFetchReport;
use mtg_cli::fetch::scryfall::{BulkAction, BulkOutcome};
use mtg_cli::ingest::{IngestOptions, ingest};
use mtg_cli::status::{database_report, status};
use mtg_cli::{decks_summary, run, scryfall_summary};
use mtg_db::Database;
use sqlx::PgPool;

use common::{fixture_cache, progress};

fn ingest_cli(cache: &Cache, dry_run: bool) -> Cli {
    Cli {
        cache: Some(cache.root().to_path_buf()),
        database_url: None,
        command: Command::Ingest(IngestArgs {
            dry_run,
            force: false,
            batch_size: 500,
        }),
    }
}

#[tokio::test]
async fn a_dry_run_needs_no_database_but_an_ingest_does() {
    let dir = tempfile::tempdir().unwrap();
    let cache = fixture_cache(dir.path());
    let (progress, _) = progress();

    let summary = run(ingest_cli(&cache, true), progress.clone())
        .await
        .unwrap();
    assert!(
        summary.starts_with("ingest: dry run, nothing written: cards "),
        "{summary}"
    );
    assert!(!summary.contains('\n'));

    let error = run(ingest_cli(&cache, false), progress).await.unwrap_err();
    assert!(error.to_string().contains("DATABASE_URL"), "{error}");
}

#[tokio::test]
async fn status_without_a_database_still_describes_the_cache() {
    let dir = tempfile::tempdir().unwrap();
    let cache = fixture_cache(dir.path());

    let text = status(&cache, None).await.unwrap();

    assert!(
        text.starts_with(&format!("cache: {}", dir.path().display())),
        "{text}"
    );
    assert!(
        text.contains("scryfall oracle_cards: scryfall/oracle-cards-fixture.json.gz"),
        "{text}"
    );
    assert!(
        text.contains("scryfall rulings: scryfall/rulings-fixture.json"),
        "{text}"
    );
    assert!(text.contains("mtgjson decks: 1 files"), "{text}");
    assert!(text.contains("database: DATABASE_URL is not set"), "{text}");
}

#[sqlx::test]
async fn status_shows_the_rows_and_the_latest_runs_of_a_loaded_database(pool: PgPool) {
    let dir = tempfile::tempdir().unwrap();
    let cache = fixture_cache(dir.path());
    let db = Database::from_pool(pool);
    let (progress, _) = progress();
    let options = IngestOptions {
        force: false,
        batch_size: 500,
    };
    ingest(&cache, &db, &options, &progress).await.unwrap();

    let text = database_report(&db, "db.example:5432/magic").await;

    assert!(
        text.starts_with("database: db.example:5432/magic\n"),
        "{text}"
    );
    let rows = db.row_counts().await.unwrap();
    assert!(
        text.contains(&format!(
            "rows: cards {}, printings {}, rulings {}, precon decks 1 (5 lines)",
            rows.cards, rows.printings, rows.rulings
        )),
        "{text}"
    );
    assert!(text.contains("latest import runs:"), "{text}");
    for source in ["scryfall_cards", "scryfall_rulings", "mtgjson_decks"] {
        assert!(
            text.contains(&format!("{source} ")),
            "{source} missing from\n{text}"
        );
    }
    assert_eq!(text.matches("succeeded").count(), 3, "{text}");
    assert!(text.contains(", input "), "{text}");
    assert!(
        text.contains("compiled rules: 0 cards (0 full, 0 partial); `mtg-cli compile` writes them"),
        "{text}"
    );

    mtg_cli::compile::compile(
        &db,
        &mtg_cli::args::CompileArgs {
            all: true,
            card: None,
        },
    )
    .await
    .unwrap();
    let text = database_report(&db, "db.example:5432/magic").await;
    assert!(
        text.contains("compiled rules: 8 cards (5 full, 3 partial)"),
        "{text}"
    );
}

#[sqlx::test(migrations = false)]
async fn status_of_an_empty_database_says_the_schema_is_missing(pool: PgPool) {
    let text = database_report(&Database::from_pool(pool.clone()), "db.example:5432/magic").await;
    assert!(text.contains("the schema is not applied yet"), "{text}");

    let db = Database::from_pool(pool);
    db.migrate().await.unwrap();
    let text = database_report(&db, "db.example:5432/magic").await;
    assert!(
        text.contains("rows: cards 0, printings 0, rulings 0, precon decks 0 (0 lines)"),
        "{text}"
    );
    assert!(text.contains("no import runs yet"), "{text}");
}

#[test]
fn summaries_are_one_line_and_say_what_happened() {
    let outcome = |kind, action| BulkOutcome {
        kind,
        action,
        record: mtg_cli::cache::BulkRecord {
            file: mtg_cli::cache::FileRecord {
                file: "scryfall/x".into(),
                url: "http://x".into(),
                size: 25_012_034,
                sha256: "ab".repeat(32),
                fetched_at: "2026-10-19T08:00:00Z".into(),
            },
            updated_at: "u".into(),
        },
    };
    let scryfall = scryfall_summary(
        &[
            outcome("oracle_cards", BulkAction::Downloaded { resumed_from: 0 }),
            outcome("rulings", BulkAction::UpToDate),
        ],
        Duration::from_millis(2500),
    );
    assert_eq!(
        scryfall,
        "fetch scryfall: oracle_cards downloaded (23.9 MiB), rulings up to date (2.5 s)"
    );
    let decks = decks_summary(
        &DeckFetchReport {
            selected: 198,
            downloaded: 190,
            not_modified: 5,
            current: 2,
            missing: vec!["X".into()],
        },
        Duration::from_secs(62),
    );
    assert_eq!(
        decks,
        "fetch decks: 198 selected, 190 downloaded, 5 unchanged, 2 current, 1 missing (62.0 s)"
    );
}
