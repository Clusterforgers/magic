//! Goal: `coverage --all` ranks the cards that are legal in Commander by the EDHREC rank of the cached Oracle Cards file: the pool, the rank buckets, the three rankings and the list written for the sweep agents; it refuses a cache without ranks or without the file, and a database without the schema or without legal cards.

use crate::common;

use std::path::Path;

use mtg_cli::args::CoverageArgs;
use mtg_cli::cache::Cache;
use mtg_cli::coverage_all::coverage_all;
use mtg_cli::error::CliError;
use mtg_cli::ingest::{IngestOptions, ingest};
use mtg_db::Database;
use sqlx::PgPool;

use common::{fixture_cache, progress, put_ranks};

/// A database with the card fixtures and a cache whose file ranks Sol Ring 1, Arcane Signet 2, Lightning Bolt 6, Forest 50,
/// Jace 300 and, though they are not legal in Commander, Little Girl 4 and the Goblin token 5.
async fn ranked(pool: PgPool, dir: &Path) -> (Database, Cache) {
    let cache = fixture_cache(dir);
    let ranks = [
        ("Sol Ring", Some(1)),
        ("Arcane Signet", Some(2)),
        ("Little Girl", Some(4)),
        ("Goblin", Some(5)),
        ("Lightning Bolt", Some(6)),
        ("Forest", Some(50)),
        ("Jace, the Mind Sculptor", Some(300)),
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

fn args(top: usize, out: Option<&Path>) -> CoverageArgs {
    CoverageArgs {
        deck: None,
        top,
        all: true,
        out: out.map(Path::to_path_buf),
    }
}

#[sqlx::test]
async fn the_pool_is_the_cards_legal_in_commander_with_the_ranks_of_the_cached_file(pool: PgPool) {
    let dir = tempfile::tempdir().unwrap();
    let (db, cache) = ranked(pool, dir.path()).await;

    let text = coverage_all(&db, &cache, &args(20, None)).await.unwrap();

    // Little Girl and the token are ranked in the file but not legal, so only the other five count.
    assert!(
        text.starts_with(
            "coverage: every Commander-legal card, 16 distinct cards, 5 with a rank\n"
        ),
        "{text}"
    );
    for wanted in [
        "state ",
        "verified cards by popularity rank",
        "top 100",
        "all ranked",
        "no rank",
        "blockers that alone hold back the most cards",
        "blockers that hold back the most cards in all",
        "blockers that hold back the most popularity weight",
    ] {
        assert!(text.contains(wanted), "{wanted} missing from\n{text}");
    }
    let row = |label: &str| {
        let line = text.lines().find(|l| l.trim_start().starts_with(label));
        let fields: Vec<&str> = line.unwrap_or_default().split_whitespace().collect();
        fields[fields.len() - 3..fields.len() - 1]
            .iter()
            .map(|n| n.parse::<u32>().unwrap())
            .collect::<Vec<_>>()
    };
    // Cards and verified cards of the five ranked cards and of the rest: the Forest is verified whatever the engine runs.
    assert_eq!(row("all ranked")[0], 5, "{text}");
    assert_eq!(row("no rank")[0], 11, "{text}");
    assert!(row("all ranked")[1] >= 1, "{text}");
    assert!(text.lines().count() <= 100, "{text}");
}

#[sqlx::test]
async fn the_sweep_list_has_the_not_verified_cards_in_rank_order(pool: PgPool) {
    let dir = tempfile::tempdir().unwrap();
    let (db, cache) = ranked(pool, dir.path()).await;
    let file = dir.path().join("sweep.tsv");

    let text = coverage_all(&db, &cache, &args(20, Some(&file)))
        .await
        .unwrap();

    let written = std::fs::read_to_string(&file).unwrap();
    assert!(written.ends_with('\n'), "{written:?}");
    let rows: Vec<Vec<&str>> = written.lines().map(|l| l.split('\t').collect()).collect();
    assert!(rows.iter().all(|row| row.len() == 4), "{written}");
    let ranks: Vec<u32> = rows.iter().map(|row| row[0].parse().unwrap()).collect();
    assert!(ranks.windows(2).all(|pair| pair[0] < pair[1]), "{written}");
    assert!(rows.iter().all(|row| row[1] != "verified"), "{written}");
    assert!(rows.iter().all(|row| !row[3].is_empty()), "{written}");
    // Jace is far from verified; the Forest is verified; Little Girl and the token are not in the pool.
    let named = |name: &str| rows.iter().find(|row| row[2] == name);
    assert_eq!(
        named("Jace, the Mind Sculptor").map(|row| row[0]),
        Some("300")
    );
    assert!(
        named("Forest").is_none() && named("Little Girl").is_none(),
        "{written}"
    );
    assert!(named("Goblin").is_none(), "{written}");
    assert!(
        text.ends_with(&format!(
            "{} not verified cards of the top 2000 written to {}",
            rows.len(),
            file.display()
        )),
        "{text}"
    );
}

#[sqlx::test]
async fn top_cuts_each_of_the_three_rankings(pool: PgPool) {
    let dir = tempfile::tempdir().unwrap();
    let (db, cache) = ranked(pool, dir.path()).await;

    let text = coverage_all(&db, &cache, &args(1, None)).await.unwrap();

    let rows = |title: &str| {
        let section = text.split(title).nth(1).unwrap_or_default();
        let section = section.split("\n\n").next().unwrap_or_default();
        // The title line, the column header and the rows.
        section.lines().count().saturating_sub(2)
    };
    assert_eq!(rows("hold back the most cards in all"), 1, "{text}");
    assert_eq!(rows("alone hold back the most cards"), 1, "{text}");
    assert_eq!(rows("the most popularity weight"), 1, "{text}");
}

#[sqlx::test]
async fn a_file_without_ranks_is_refused(pool: PgPool) {
    let dir = tempfile::tempdir().unwrap();
    let (db, cache) = ranked(pool, dir.path()).await;
    put_ranks(&cache, &[]);

    let error = coverage_all(&db, &cache, &args(20, None))
        .await
        .unwrap_err();

    assert!(matches!(error, CliError::Config(_)), "{error}");
    assert!(error.to_string().contains("edhrec_rank"), "{error}");
}

#[sqlx::test]
async fn a_cache_without_the_oracle_file_says_how_to_fetch_it(pool: PgPool) {
    let dir = tempfile::tempdir().unwrap();

    let error = coverage_all(
        &Database::from_pool(pool),
        &Cache::new(dir.path()),
        &args(20, None),
    )
    .await
    .unwrap_err();

    assert!(matches!(error, CliError::Config(_)), "{error}");
    assert!(error.to_string().contains("fetch scryfall"), "{error}");
}

#[sqlx::test]
async fn a_database_without_legal_cards_says_how_to_load_them(pool: PgPool) {
    let dir = tempfile::tempdir().unwrap();
    let cache = fixture_cache(dir.path());

    let error = coverage_all(&Database::from_pool(pool), &cache, &args(20, None))
        .await
        .unwrap_err();

    assert!(matches!(error, CliError::Config(_)), "{error}");
    assert!(error.to_string().contains("mtg-cli ingest"), "{error}");
}

#[sqlx::test(migrations = false)]
async fn a_database_without_a_schema_says_how_to_apply_it(pool: PgPool) {
    let dir = tempfile::tempdir().unwrap();
    let cache = fixture_cache(dir.path());

    let error = coverage_all(&Database::from_pool(pool), &cache, &args(20, None))
        .await
        .unwrap_err();

    assert!(matches!(error, CliError::Config(_)), "{error}");
    assert!(
        error.to_string().contains("schema is not applied"),
        "{error}"
    );
}
