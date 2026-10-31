//! Goal: `reads` prints the stored verdicts newest first with the card, the state of its draft and the account (the server's own as `server`), keeps only the wrong ones with `--wrong`, cuts the table at `--limit` and flags with `--quick` the accounts that judged different cards less than four seconds apart; it says so when there is nothing to print and when the schema is missing.
#![allow(clippy::unwrap_used)]

use crate::common;

use std::path::Path;

use mtg_cli::args::ReadsArgs;
use mtg_cli::ingest::{IngestOptions, ingest};
use mtg_cli::reads::reads;
use mtg_db::{Database, DraftKey};
use sqlx::PgPool;
use sqlx::types::uuid::Uuid;

use common::{fixture_cache, progress, store_draft};

async fn seeded(pool: PgPool, dir: &Path) -> Database {
    let cache = fixture_cache(dir);
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

fn args(wrong: bool, quick: bool, limit: u32) -> ReadsArgs {
    ReadsArgs {
        wrong,
        quick,
        limit,
    }
}

fn user(n: u128) -> Uuid {
    Uuid::from_u128(n)
}

/// Appends a verdict by `by` (the server when `None`) given at `at`, which the repository cannot set.
async fn verdict(
    db: &Database,
    by: Option<Uuid>,
    key: &DraftKey,
    wrong: bool,
    note: Option<&str>,
    at: &str,
) {
    sqlx::query(
        "INSERT INTO card_reads (user_id, oracle_id, read_hash, verdict, note, created_at)
         VALUES ($1, $2::uuid, $3, $4::read_verdict, $5, $6::timestamptz)",
    )
    .bind(by)
    .bind(key.oracle_id.as_str())
    .bind(&key.read_hash)
    .bind(if wrong { "wrong" } else { "correct" })
    .bind(note)
    .bind(at)
    .execute(db.pool())
    .await
    .unwrap();
}

/// A database with drafts of Arcane Signet (read correct twice) and Atraxa (read wrong, and wrong in a game).
async fn judged(pool: PgPool, dir: &Path) -> (Database, DraftKey, DraftKey) {
    let db = seeded(pool, dir).await;
    let signet = store_draft(&db, "Arcane Signet").await;
    let atraxa = store_draft(&db, "Atraxa, Praetors' Voice").await;
    verdict(
        &db,
        Some(user(1)),
        &signet,
        false,
        None,
        "2026-10-31 10:00:00+00",
    )
    .await;
    verdict(
        &db,
        Some(user(2)),
        &signet,
        false,
        None,
        "2026-10-31 10:05:00+00",
    )
    .await;
    let note = Some("the upkeep trigger\nnever fires");
    verdict(
        &db,
        Some(user(2)),
        &atraxa,
        true,
        note,
        "2026-10-31 10:06:00+00",
    )
    .await;
    let note = Some("ResolveError in a game");
    verdict(&db, None, &atraxa, true, note, "2026-10-31 11:00:00+00").await;
    (db, signet, atraxa)
}

#[sqlx::test]
async fn the_table_lists_the_verdicts_newest_first_with_the_state_of_the_draft(pool: PgPool) {
    let dir = tempfile::tempdir().unwrap();
    let (db, _, _) = judged(pool, dir.path()).await;

    let text = reads(&db, &args(false, false, 50)).await.unwrap();

    let lines: Vec<&str> = text.lines().collect();
    assert!(
        lines[0].starts_with("reads (newest 4 of 4 verdicts;"),
        "{text}"
    );
    assert!(lines[1].starts_with("time (UTC)"), "{text}");
    let cells = |line: &str| -> Vec<String> {
        line.split("  ")
            .map(str::trim)
            .filter(|cell| !cell.is_empty())
            .map(str::to_owned)
            .collect()
    };
    let server = cells(lines[2]);
    assert_eq!(server[0], "2026-10-31 11:00:00", "{text}");
    assert_eq!(
        &server[1..5],
        ["Atraxa, Praetors' Voice", "wrong", "server", "wrong"]
    );
    assert_eq!(server[5], "ResolveError in a game");
    let second = cells(lines[3]);
    assert_eq!(second[0], "2026-10-31 10:06:00", "{text}");
    assert_eq!(second[3], user(2).to_string());
    assert_eq!(second[5], "the upkeep trigger never fires", "{text}");
    let newest_correct = cells(lines[4]);
    assert_eq!(
        &newest_correct[1..3],
        ["Arcane Signet", "approved"],
        "{text}"
    );
    assert_eq!(lines.len(), 6, "{text}");
}

#[sqlx::test]
async fn wrong_keeps_the_wrong_verdicts_and_limit_cuts_the_table_not_its_count(pool: PgPool) {
    let dir = tempfile::tempdir().unwrap();
    let (db, _, _) = judged(pool, dir.path()).await;

    let wrong = reads(&db, &args(true, false, 50)).await.unwrap();
    let one = reads(&db, &args(false, false, 1)).await.unwrap();

    assert!(
        wrong.starts_with("reads (newest 2 of 2 verdicts;"),
        "{wrong}"
    );
    assert!(
        wrong.lines().skip(2).all(|line| line.contains("  wrong")),
        "{wrong}"
    );
    assert!(!wrong.contains("Arcane Signet"), "{wrong}");
    assert!(one.starts_with("reads (newest 1 of 4 verdicts;"), "{one}");
    assert_eq!(one.lines().count(), 3, "{one}");
    assert!(one.contains("server"), "{one}");
}

#[sqlx::test]
async fn a_verdict_on_a_replaced_draft_is_old(pool: PgPool) {
    let dir = tempfile::tempdir().unwrap();
    let (db, _, atraxa) = judged(pool, dir.path()).await;
    db.drafts().retire(&[atraxa]).await.unwrap();

    let text = reads(&db, &args(true, false, 50)).await.unwrap();

    assert!(
        text.lines().skip(2).all(|line| line.contains("  old  ")),
        "{text}"
    );
}

#[sqlx::test]
async fn quick_flags_the_accounts_that_judge_different_cards_within_four_seconds(pool: PgPool) {
    let dir = tempfile::tempdir().unwrap();
    let db = seeded(pool, dir.path()).await;
    let signet = store_draft(&db, "Arcane Signet").await;
    let atraxa = store_draft(&db, "Atraxa, Praetors' Voice").await;
    // Account 1 judges two cards 2 s apart (the second wrong); account 2 takes 10 s and account 3 judges one card twice.
    verdict(
        &db,
        Some(user(1)),
        &signet,
        false,
        None,
        "2026-10-31 10:00:00+00",
    )
    .await;
    verdict(
        &db,
        Some(user(1)),
        &atraxa,
        true,
        Some("nope"),
        "2026-10-31 10:00:02+00",
    )
    .await;
    verdict(
        &db,
        Some(user(2)),
        &signet,
        false,
        None,
        "2026-10-31 10:00:00+00",
    )
    .await;
    verdict(
        &db,
        Some(user(2)),
        &atraxa,
        false,
        None,
        "2026-10-31 10:00:10+00",
    )
    .await;
    verdict(
        &db,
        Some(user(3)),
        &signet,
        false,
        None,
        "2026-10-31 10:00:00+00",
    )
    .await;
    verdict(
        &db,
        Some(user(3)),
        &signet,
        false,
        None,
        "2026-10-31 10:00:01+00",
    )
    .await;

    let quick = reads(&db, &args(false, true, 50)).await.unwrap();
    let quick_wrong = reads(&db, &args(true, true, 50)).await.unwrap();

    let (accounts, table) = quick.split_once("\nreads (").unwrap();
    assert!(
        accounts.starts_with("1 account gave verdicts on different cards less than 4 s apart"),
        "{quick}"
    );
    let rows: Vec<&str> = accounts.lines().skip(2).collect();
    assert_eq!(rows.len(), 1, "{quick}");
    assert!(rows[0].starts_with(&user(1).to_string()), "{quick}");
    assert!(rows[0].ends_with("    2      2"), "{quick}");
    assert!(
        table.starts_with("newest 2 of 2 quick verdicts;"),
        "{quick}"
    );
    assert_eq!(table.lines().count(), 4, "{quick}");
    assert!(!quick.contains(&user(2).to_string()) && !quick.contains(&user(3).to_string()));
    // The correct verdict beside the wrong one is why the account is flagged; only the wrong one is listed.
    let (_, table) = quick_wrong.split_once("\nreads (").unwrap();
    assert!(
        table.starts_with("newest 1 of 1 quick verdicts;"),
        "{quick_wrong}"
    );
}

#[sqlx::test]
async fn with_nothing_to_print_the_command_says_so(pool: PgPool) {
    let dir = tempfile::tempdir().unwrap();
    let db = seeded(pool, dir.path()).await;
    let signet = store_draft(&db, "Arcane Signet").await;

    let none = reads(&db, &args(false, false, 50)).await.unwrap();
    verdict(
        &db,
        Some(user(1)),
        &signet,
        false,
        None,
        "2026-10-31 10:00:00+00",
    )
    .await;
    let no_wrong = reads(&db, &args(true, false, 50)).await.unwrap();
    let no_quick = reads(&db, &args(false, true, 50)).await.unwrap();

    assert_eq!(none, "no read verdict is stored yet");
    assert_eq!(no_wrong, "no wrong verdict is stored yet");
    assert_eq!(
        no_quick,
        "no account gave verdicts on different cards less than 4 seconds apart"
    );
}

#[sqlx::test(migrations = false)]
async fn a_database_without_the_schema_asks_for_ingest(pool: PgPool) {
    let db = Database::from_pool(pool);

    let error = reads(&db, &args(false, false, 50)).await.unwrap_err();

    assert!(error.to_string().contains("mtg-cli ingest"), "{error}");
}
