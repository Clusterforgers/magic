//! Goal: `coverage` reads the stored precons and reports on their cards: all of them or those a code or a name part selects, the counts of cards and copies, the decks and the blockers; it refuses a selection that matches nothing and a database without precons.

use crate::common;

use mtg_cli::args::CoverageArgs;
use mtg_cli::cache::Cache;
use mtg_cli::coverage::coverage;
use mtg_cli::error::CliError;
use mtg_cli::ingest::{IngestOptions, ingest};
use mtg_db::Database;
use serde_json::{Value, json};
use sqlx::PgPool;

use common::{fixture, fixture_cache, progress, put_deck};

/// The deck fixture as a second precon: another code and name, and a main board of Sol Ring and ten Forests.
fn second_deck() -> Vec<u8> {
    let mut deck: Value = serde_json::from_slice(&fixture("mtgjson_deck_commander.json"))
        .unwrap_or_else(|e| panic!("the deck fixture reads: {e}"));
    let data = &mut deck["data"];
    data["code"] = json!("C17");
    data["name"] = json!("Second Deck");
    data["releaseDate"] = json!("2017-11-10");
    let board = data["mainBoard"].clone();
    let mut forests = board[2].clone();
    forests["count"] = json!(10);
    data["mainBoard"] = json!([board[0].clone(), forests]);
    serde_json::to_vec(&deck).unwrap_or_else(|e| panic!("the deck writes: {e}"))
}

/// A database holding the card fixtures and two small precons: the fixture deck (40 cards) and another (12).
async fn two_precons(pool: PgPool, cache_dir: &std::path::Path) -> Database {
    let cache: Cache = fixture_cache(cache_dir);
    put_deck(&cache, "SecondDeck_C17", &second_deck());
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

fn args(deck: Option<&str>) -> CoverageArgs {
    CoverageArgs {
        deck: deck.map(str::to_owned),
        top: 20,
        all: false,
        out: None,
    }
}

#[sqlx::test]
async fn the_report_covers_every_stored_precon(pool: PgPool) {
    let dir = tempfile::tempdir().unwrap();
    let db = two_precons(pool, dir.path()).await;

    let text = coverage(&db, &args(None)).await.unwrap();

    assert!(
        text.starts_with("coverage: 2 precons, 5 distinct cards, 52 copies\n"),
        "{text}"
    );
    for wanted in [
        "state ",
        "best precons by the share",
        "C16 Breed Lethality",
        "C17 Second Deck",
        "blockers that alone hold back",
        "blockers that hold back the most cards in all",
    ] {
        assert!(text.contains(wanted), "{wanted} missing from\n{text}");
    }
    assert!(text.lines().count() <= 80, "{text}");
    // The Forests (46 copies) are verified whatever the engine runs.
    let verified = text
        .lines()
        .find(|line| line.starts_with("verified"))
        .unwrap();
    let copies: u32 = verified.split_whitespace().nth(3).unwrap().parse().unwrap();
    assert!(copies >= 46, "{verified}");
}

#[sqlx::test]
async fn a_code_or_a_name_part_selects_the_precons(pool: PgPool) {
    let dir = tempfile::tempdir().unwrap();
    let db = two_precons(pool, dir.path()).await;
    // What is asked, the first line of the report, the deck it shows and the one it leaves out.
    let cases = [
        (
            "C17",
            "coverage: 1 precon, 3 distinct cards, 12 copies",
            "C17 Second Deck",
            "Breed",
        ),
        (
            "c17",
            "coverage: 1 precon, 3 distinct cards, 12 copies",
            "C17 Second Deck",
            "Breed",
        ),
        (
            "LETHAL",
            "coverage: 1 precon, 5 distinct cards, 40 copies",
            "C16 Breed Lethality",
            "Second",
        ),
        (
            "second d",
            "coverage: 1 precon, 3 distinct cards, 12 copies",
            "C17 Second Deck",
            "Breed",
        ),
    ];
    for (wanted, header, shown, hidden) in cases {
        let text = coverage(&db, &args(Some(wanted))).await.unwrap();

        assert!(text.starts_with(&format!("{header}\n")), "{wanted}: {text}");
        assert!(text.contains(shown), "{wanted}: {text}");
        assert!(!text.contains(hidden), "{wanted}: {text}");
    }
}

#[sqlx::test]
async fn a_selection_that_matches_nothing_is_refused(pool: PgPool) {
    let dir = tempfile::tempdir().unwrap();
    let db = two_precons(pool, dir.path()).await;

    let error = coverage(&db, &args(Some("no such deck")))
        .await
        .unwrap_err();

    assert!(matches!(error, CliError::Config(_)), "{error}");
    assert!(error.to_string().contains("no such deck"), "{error}");
}

#[sqlx::test]
async fn a_database_without_precons_says_how_to_load_them(pool: PgPool) {
    let error = coverage(&Database::from_pool(pool), &args(None))
        .await
        .unwrap_err();

    assert!(matches!(error, CliError::Config(_)), "{error}");
    assert!(error.to_string().contains("mtg-cli ingest"), "{error}");
}

#[sqlx::test(migrations = false)]
async fn a_database_without_a_schema_says_how_to_apply_it(pool: PgPool) {
    let error = coverage(&Database::from_pool(pool), &args(None))
        .await
        .unwrap_err();

    assert!(matches!(error, CliError::Config(_)), "{error}");
    assert!(
        error.to_string().contains("schema is not applied"),
        "{error}"
    );
}
