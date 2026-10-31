//! Goal: `coverage` reads the stored drafts and verdicts: an approved draft counts as `Read`, a pending or stale one as waiting for a read, a wrong one changes nothing, in the precon table and with `--all`; `--deck` ends in the list of the deck's cards that are neither verified nor read, most played first with the first blocker.
#![allow(clippy::unwrap_used)]

use crate::common;

use std::path::Path;

use mtg_cli::args::CoverageArgs;
use mtg_cli::cache::Cache;
use mtg_cli::coverage::{coverage, report};
use mtg_cli::coverage_all::coverage_all;
use mtg_cli::ingest::{IngestOptions, ingest};
use mtg_db::{Database, DraftKey, NewDraft, Verdict};
use sqlx::PgPool;
use sqlx::types::uuid::Uuid;

use common::{fixture, fixture_cache, progress, put_deck, put_ranks, store_draft};

/// The deck fixture as a second precon, C17, of cards the corpus does not prove: Jace, Lim-Dûl's Vault and Pyroblast, and
/// ten Forests.
fn draft_deck() -> Vec<u8> {
    let mut deck: serde_json::Value =
        serde_json::from_slice(&fixture("mtgjson_deck_commander.json")).unwrap();
    let data = &mut deck["data"];
    data["code"] = "C17".into();
    data["name"] = "Draft Deck".into();
    let board = data["mainBoard"].clone();
    let card = |name: &str, oracle_id: &str| {
        let mut entry = board[0].clone();
        entry["name"] = name.into();
        entry["identifiers"]["scryfallOracleId"] = oracle_id.into();
        entry
    };
    let mut forests = board[2].clone();
    forests["count"] = 10.into();
    data["mainBoard"] = serde_json::json!([
        card(
            "Jace, the Mind Sculptor",
            "ac249b6c-e009-44db-a91d-6178063e4a57"
        ),
        card("Lim-Dûl's Vault", "1f12ed65-1796-4bd0-9092-1851b0468749"),
        card("Pyroblast", "89537e9a-56df-422a-a108-c6faadd4e751"),
        forests,
    ]);
    serde_json::to_vec(&deck).unwrap()
}

/// A database with the card fixtures and the precons C16 and C17, with the ranks Sol Ring 1, Lim-Dûl's Vault 2, Pyroblast 10,
/// Jace 30 and Forest 50 in the cached file.
async fn seeded(pool: PgPool, dir: &Path) -> (Database, Cache) {
    let cache = fixture_cache(dir);
    put_deck(&cache, "DraftDeck_C17", &draft_deck());
    let ranks = [
        ("Sol Ring", Some(1)),
        ("Lim-Dûl's Vault", Some(2)),
        ("Pyroblast", Some(10)),
        ("Jace, the Mind Sculptor", Some(30)),
        ("Forest", Some(50)),
    ];
    put_ranks(&cache, &ranks);
    let db = Database::from_pool(pool);
    let (progress, _) = progress();
    let options = IngestOptions {
        force: false,
        batch_size: 500,
    };
    ingest(&cache, &db, &options, &progress)
        .await
        .unwrap_or_else(|e| panic!("ingest runs: {e}"));
    (db, cache)
}

fn args(deck: Option<&str>, all: bool) -> CoverageArgs {
    CoverageArgs {
        deck: deck.map(str::to_owned),
        top: 20,
        all,
        out: None,
    }
}

/// The cards of each state row of a report: verified, read, waiting, partial, manual, other.
fn states(text: &str) -> [u32; 6] {
    let row = |name: &str| {
        let line = text.lines().find(|l| l.starts_with(&format!("{name} ")));
        let fields: Vec<&str> = line.unwrap_or_default().split_whitespace().collect();
        fields
            .get(1)
            .and_then(|n| n.parse().ok())
            .unwrap_or(u32::MAX)
    };
    ["verified", "read", "waiting", "partial", "manual", "other"].map(row)
}

async fn judge(db: &Database, key: &DraftKey, verdict: Verdict) {
    let note = (verdict == Verdict::Wrong).then_some("does the wrong thing");
    db.reads()
        .append(Some(Uuid::from_u128(1)), key, verdict, note)
        .await
        .unwrap()
        .unwrap();
}

#[sqlx::test]
async fn an_approved_draft_is_read_and_a_pending_one_waits_for_a_read(pool: PgPool) {
    let dir = tempfile::tempdir().unwrap();
    let (db, _) = seeded(pool, dir.path()).await;
    let before = states(&coverage(&db, &args(None, false)).await.unwrap());
    let vault = store_draft(&db, "Lim-Dûl's Vault").await;
    judge(&db, &vault, Verdict::Correct).await;
    store_draft(&db, "Pyroblast").await;

    let text = coverage(&db, &args(None, false)).await.unwrap();

    let after = states(&text);
    assert_eq!(before[1..3], [0, 0], "{before:?}");
    assert_eq!(after[1..3], [1, 1], "{text}");
    // Nothing else moves: the two cards leave partial, manual or other, and the total stays.
    assert_eq!(after[0], before[0], "{text}");
    assert_eq!(
        after.iter().sum::<u32>(),
        before.iter().sum::<u32>(),
        "{text}"
    );
    assert_eq!(
        after[3..].iter().sum::<u32>() + 2,
        before[3..].iter().sum::<u32>()
    );
}

#[sqlx::test]
async fn a_wrong_draft_changes_nothing_and_a_draft_read_on_an_old_hash_waits_again(pool: PgPool) {
    let dir = tempfile::tempdir().unwrap();
    let (db, _) = seeded(pool, dir.path()).await;
    let before = states(&coverage(&db, &args(None, false)).await.unwrap());
    let vault = store_draft(&db, "Lim-Dûl's Vault").await;
    judge(&db, &vault, Verdict::Wrong).await;
    // Pyroblast's draft was approved on a hash the card no longer has: its text or draft changed since.
    let pyroblast = store_draft(&db, "Pyroblast").await;
    let rules = sqlx::query_scalar::<_, serde_json::Value>(
        "SELECT rules FROM card_drafts WHERE oracle_id = $1::uuid",
    )
    .bind(pyroblast.oracle_id.as_str())
    .fetch_one(db.pool())
    .await
    .unwrap();
    let old = NewDraft {
        oracle_id: pyroblast.oracle_id.clone(),
        read_hash: "0123456789abcdef0123456789abcdef".to_owned(),
        rules,
    };
    db.drafts()
        .insert(std::slice::from_ref(&old))
        .await
        .unwrap();
    let old_key = DraftKey {
        oracle_id: old.oracle_id,
        read_hash: old.read_hash,
    };
    judge(&db, &old_key, Verdict::Correct).await;

    let text = coverage(&db, &args(None, false)).await.unwrap();

    let after = states(&text);
    // The wrong draft is by hand as before; the stale approval is a draft to read again.
    assert_eq!(after[1..3], [0, 1], "{text}");
    assert_eq!(after[0], before[0], "{text}");
    assert_eq!(
        after.iter().sum::<u32>(),
        before.iter().sum::<u32>(),
        "{text}"
    );
}

#[sqlx::test]
async fn a_draft_for_a_verified_card_changes_nothing(pool: PgPool) {
    let dir = tempfile::tempdir().unwrap();
    let (db, _) = seeded(pool, dir.path()).await;
    let before = coverage(&db, &args(None, false)).await.unwrap();
    let forest = store_draft(&db, "Forest").await;
    judge(&db, &forest, Verdict::Correct).await;

    let after = coverage(&db, &args(None, false)).await.unwrap();

    assert_eq!(after, before);
}

#[sqlx::test]
async fn the_all_report_counts_read_and_waiting_cards_too(pool: PgPool) {
    let dir = tempfile::tempdir().unwrap();
    let (db, cache) = seeded(pool, dir.path()).await;
    let before = states(&coverage_all(&db, &cache, &args(None, true)).await.unwrap());
    let vault = store_draft(&db, "Lim-Dûl's Vault").await;
    judge(&db, &vault, Verdict::Correct).await;
    store_draft(&db, "Pyroblast").await;

    let text = coverage_all(&db, &cache, &args(None, true)).await.unwrap();

    let after = states(&text);
    assert_eq!(after[1..3], [1, 1], "{text}");
    assert_eq!(
        after.iter().sum::<u32>(),
        before.iter().sum::<u32>(),
        "{text}"
    );
    assert_eq!(after[0], before[0], "{text}");
}

#[sqlx::test]
async fn the_deck_list_has_the_cards_to_draft_most_played_first(pool: PgPool) {
    let dir = tempfile::tempdir().unwrap();
    let (db, cache) = seeded(pool, dir.path()).await;
    let vault = store_draft(&db, "Lim-Dûl's Vault").await;
    judge(&db, &vault, Verdict::Correct).await;
    store_draft(&db, "Pyroblast").await;

    let text = report(&db, &cache, &args(Some("c17"), false))
        .await
        .unwrap();

    let (table, list) = text.split_once("\n\ncards to draft: ").unwrap();
    assert!(
        table.starts_with("coverage: 1 precon, 5 distinct cards,"),
        "{text}"
    );
    let mut lines = list.lines();
    assert_eq!(
        lines.next().unwrap().split(" (").next().unwrap(),
        "2 of 5 distinct cards are neither verified nor read, most played first",
        "{text}"
    );
    assert!(lines.next().unwrap().ends_with("first blocker"), "{text}");
    let rows: Vec<Vec<&str>> = lines.map(|l| l.split_whitespace().collect()).collect();
    // The read Vault, the verified Forest and the commander are not listed; the other two come in the order of their ranks, each
    // with its first blocker.
    assert_eq!(rows.len(), 2, "{text}");
    assert_eq!(rows[0][..3], ["10", "waiting", "Pyroblast"], "{text}");
    assert_eq!(rows[0][3], "modal:", "{text}");
    assert_eq!((rows[1][0], rows[1][2]), ("30", "Jace,"), "{text}");
    assert!(
        !["waiting", "verified", "read"].contains(&rows[1][1]),
        "{text}"
    );
    assert!(rows[1].contains(&"loyalty:"), "{text}");
}

#[sqlx::test]
async fn a_deck_with_nothing_left_to_draft_says_so(pool: PgPool) {
    let dir = tempfile::tempdir().unwrap();
    let (db, cache) = seeded(pool, dir.path()).await;
    for name in ["Jace, the Mind Sculptor", "Lim-Dûl's Vault", "Pyroblast"] {
        let key = store_draft(&db, name).await;
        judge(&db, &key, Verdict::Correct).await;
    }

    let text = report(&db, &cache, &args(Some("c17"), false))
        .await
        .unwrap();

    assert!(
        text.ends_with("cards to draft: none, all 5 distinct cards are verified or read"),
        "{text}"
    );
}

#[sqlx::test]
async fn without_a_deck_the_report_has_no_list_and_a_deck_needs_the_ranks(pool: PgPool) {
    let dir = tempfile::tempdir().unwrap();
    let (db, cache) = seeded(pool, dir.path()).await;

    let all = report(&db, &cache, &args(None, false)).await.unwrap();
    assert!(!all.contains("cards to draft"), "{all}");

    let empty = tempfile::tempdir().unwrap();
    let error = report(&db, &Cache::new(empty.path()), &args(Some("C16"), false))
        .await
        .unwrap_err();
    assert!(error.to_string().contains("Oracle Cards"), "{error}");
}
