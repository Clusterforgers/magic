//! Goal: ingest loads the cached Scryfall and MTGJSON fixtures into Postgres exactly once per input, changes nothing on a rerun or a forced reload, ties rulings and decks to the cards they were loaded against, resumes after a broken file, skips and counts NUL records, reaps runs of dead processes, and a dry run touches no database.

use crate::common;

use std::time::Duration;

use mtg_cli::cache::Cache;
use mtg_cli::error::CliError;
use mtg_cli::ingest::{IngestOptions, IngestReport, Step, dry_run, ingest};
use mtg_cli::progress::Progress;
use mtg_db::{
    CheckReport, Database, ImportError, ImportSource, RunStatus, check_cards, check_rulings,
};
use mtg_import::scryfall::cards;
use serde_json::{Value, json};
use sqlx::PgPool;

use common::{count, data, fixture, fixture_cache, gzip, progress, put_scryfall};

/// Small batches, so that every input spans several transactions.
const BATCH: usize = 3;

fn options(force: bool) -> IngestOptions {
    IngestOptions {
        force,
        batch_size: BATCH,
    }
}

/// What a dry run finds in the card fixture.
fn fixture_check() -> CheckReport {
    let fixture = fixture("scryfall_oracle_cards.json");
    check_cards(cards(fixture.as_slice())).unwrap_or_else(|e| panic!("the fixture reads: {e}"))
}

/// Usable records in the card fixture.
fn fixture_cards() -> u64 {
    fixture_check().usable
}

/// Records of the card fixture that no import can use (the file holds one deliberately broken entry).
fn fixture_skipped() -> u64 {
    fixture_check().skipped.count
}

async fn run(cache: &Cache, pool: &PgPool, force: bool) -> (IngestReport, String) {
    let (progress, log) = progress();
    let db = Database::from_pool(pool.clone());
    let report = ingest(cache, &db, &options(force), &progress)
        .await
        .unwrap_or_else(|e| panic!("ingest runs: {e}"));
    (report, log.text())
}

fn imported<R>(step: Step<R>) -> R {
    match step {
        Step::Imported(report) => report,
        other => panic!("expected an import, got {}", describe(&other)),
    }
}

fn describe<R>(step: &Step<R>) -> &'static str {
    match step {
        Step::Missing => "missing",
        Step::Unchanged => "unchanged",
        Step::Checked(_) => "checked",
        Step::Imported(_) => "imported",
    }
}

fn kinds(report: &IngestReport) -> [&'static str; 3] {
    [
        describe(&report.cards),
        describe(&report.rulings),
        describe(&report.decks),
    ]
}

#[sqlx::test]
async fn the_fixtures_are_loaded_and_each_run_records_what_it_read(pool: PgPool) {
    let dir = tempfile::tempdir().unwrap();
    let cache = fixture_cache(dir.path());

    let (report, log) = run(&cache, &pool, false).await;

    let usable = fixture_cards();
    let cards = imported(report.cards);
    assert_eq!(
        (
            cards.cards.inserted,
            cards.printings.inserted,
            cards.skipped.count
        ),
        (usable, usable, fixture_skipped())
    );
    assert_eq!(fixture_skipped(), 1, "the fixture holds one broken entry");
    let rulings = imported(report.rulings);
    let ruling_file = fixture("scryfall_rulings.json");
    let expected = check_rulings(mtg_import::scryfall::rulings(ruling_file.as_slice())).unwrap();
    assert_eq!(
        (rulings.inserted, rulings.orphaned, rulings.skipped.count),
        (expected.usable, 0, expected.skipped.count)
    );
    assert!(rulings.inserted > 0);
    let decks = imported(report.decks);
    assert_eq!(decks.decks.len(), 1);
    assert_eq!(decks.decks[0].unresolved.len(), 0);

    let db = Database::from_pool(pool.clone());
    let rows = db.row_counts().await.unwrap();
    assert_eq!((rows.cards, rows.printings), (usable, usable));
    assert_eq!((rows.precon_decks, rows.precon_entries), (1, 5));

    let manifest = cache.manifest().unwrap();
    let cards_sha = manifest.scryfall["oracle_cards"].file.sha256.clone();
    let rulings_sha = manifest.scryfall["rulings"].file.sha256.clone();
    let inputs = |source| {
        let runs = db.import_runs();
        async move {
            runs.latest_succeeded(source)
                .await
                .unwrap()
                .unwrap()
                .input
                .unwrap()
        }
    };
    assert_eq!(inputs(ImportSource::ScryfallCards).await, cards_sha);
    assert_eq!(
        inputs(ImportSource::ScryfallRulings).await,
        format!("{rulings_sha}@{cards_sha}")
    );
    assert!(
        inputs(ImportSource::MtgjsonDecks)
            .await
            .ends_with(&format!("@{cards_sha}"))
    );
    for expected in ["cards:", "rulings:", "decks:"] {
        assert!(log.contains(expected), "{expected} missing from\n{log}");
    }
}

#[sqlx::test]
async fn a_second_ingest_changes_nothing_at_all(pool: PgPool) {
    let dir = tempfile::tempdir().unwrap();
    let cache = fixture_cache(dir.path());
    run(&cache, &pool, false).await;
    let tables = data(&pool).await;
    let runs = count(&pool, "import_runs").await;

    let (again, log) = run(&cache, &pool, false).await;

    assert_eq!(kinds(&again), ["unchanged", "unchanged", "unchanged"]);
    assert_eq!(data(&pool).await, tables, "not even a timestamp moved");
    assert_eq!(
        count(&pool, "import_runs").await,
        runs,
        "no run was recorded"
    );
    assert!(log.contains("unchanged since the last import"), "{log}");
}

#[sqlx::test]
async fn a_forced_ingest_reads_everything_again_and_changes_no_row(pool: PgPool) {
    let dir = tempfile::tempdir().unwrap();
    let cache = fixture_cache(dir.path());
    run(&cache, &pool, false).await;
    let tables = data(&pool).await;
    let runs = count(&pool, "import_runs").await;

    let (forced, _) = run(&cache, &pool, true).await;

    let usable = fixture_cards();
    let cards = imported(forced.cards);
    assert_eq!(
        (
            cards.cards.inserted,
            cards.cards.updated,
            cards.cards.unchanged
        ),
        (0, 0, usable)
    );
    let rulings = imported(forced.rulings);
    assert_eq!(rulings.inserted, 0);
    let decks = imported(forced.decks);
    assert_eq!(decks.decks.len(), 1);
    assert_eq!(data(&pool).await, tables);
    assert_eq!(count(&pool, "import_runs").await, runs + 3);
}

#[sqlx::test]
async fn a_changed_cards_file_is_loaded_and_brings_the_rulings_and_decks_back(pool: PgPool) {
    let dir = tempfile::tempdir().unwrap();
    let cache = fixture_cache(dir.path());
    run(&cache, &pool, false).await;
    let original = String::from_utf8(fixture("scryfall_oracle_cards.json")).unwrap();
    let edited = original.replace(
        "Lightning Bolt deals 3 damage to any target.",
        "Lightning Bolt deals 3 damage to any target, twice.",
    );
    assert_ne!(edited, original);
    put_scryfall(
        &cache,
        "oracle_cards",
        "oracle-cards-fixture.json.gz",
        &gzip(edited.as_bytes()),
    );

    let (report, _) = run(&cache, &pool, false).await;

    let cards = imported(report.cards);
    assert_eq!(
        (
            cards.cards.inserted,
            cards.cards.updated,
            cards.cards.unchanged
        ),
        (0, 1, fixture_cards() - 1)
    );
    let rulings = imported(report.rulings);
    assert_eq!(rulings.inserted, 0, "the same rulings, read again");
    assert_eq!(imported(report.decks).decks.len(), 1);
    let text: String =
        sqlx::query_scalar("SELECT oracle_text FROM cards WHERE name = 'Lightning Bolt'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(text.ends_with(", twice."), "{text}");
}

#[sqlx::test]
async fn new_rulings_alone_load_only_the_rulings(pool: PgPool) {
    let dir = tempfile::tempdir().unwrap();
    let cache = fixture_cache(dir.path());
    run(&cache, &pool, false).await;
    let before = count(&pool, "rulings").await;
    let mut rulings: Vec<Value> =
        serde_json::from_slice(&fixture("scryfall_rulings.json")).unwrap();
    let mut extra = rulings[0].clone();
    extra["comment"] = json!("A ruling that is new.");
    rulings.push(extra);
    put_scryfall(
        &cache,
        "rulings",
        "rulings-fixture.json",
        &serde_json::to_vec(&rulings).unwrap(),
    );

    let (report, _) = run(&cache, &pool, false).await;

    assert_eq!(kinds(&report), ["unchanged", "imported", "unchanged"]);
    assert_eq!(imported(report.rulings).inserted, 1);
    assert_eq!(count(&pool, "rulings").await, before + 1);
}

#[sqlx::test]
async fn rulings_and_decks_wait_for_their_cards_and_are_loaded_again_when_the_cards_arrive(
    pool: PgPool,
) {
    let dir = tempfile::tempdir().unwrap();
    let full = fixture_cache(dir.path());
    let empty = tempfile::tempdir().unwrap();
    let cache = Cache::new(empty.path());
    common::put_scryfall(
        &cache,
        "rulings",
        "rulings-fixture.json",
        &fixture("scryfall_rulings.json"),
    );
    common::put_deck(
        &cache,
        "BreedLethality_C16",
        &fixture("mtgjson_deck_commander.json"),
    );

    let (first, log) = run(&cache, &pool, false).await;

    assert_eq!(kinds(&first), ["missing", "imported", "imported"]);
    let rulings = imported(first.rulings);
    assert_eq!((rulings.inserted, rulings.orphaned > 0), (0, true));
    let decks = imported(first.decks);
    assert_eq!(decks.decks[0].unresolved.len(), 5);
    assert!(log.contains("cards: not cached"), "{log}");
    assert!(log.contains("matched no card"), "{log}");

    let bytes = std::fs::read(full.path("scryfall/oracle-cards-fixture.json.gz")).unwrap();
    put_scryfall(
        &cache,
        "oracle_cards",
        "oracle-cards-fixture.json.gz",
        &bytes,
    );
    let (second, _) = run(&cache, &pool, false).await;

    assert_eq!(kinds(&second), ["imported", "imported", "imported"]);
    assert!(imported(second.rulings).inserted > 0);
    assert_eq!(imported(second.decks).decks[0].unresolved.len(), 0);
    let (third, _) = run(&cache, &pool, false).await;
    assert_eq!(kinds(&third), ["unchanged", "unchanged", "unchanged"]);
}

#[sqlx::test]
async fn a_card_with_a_nul_byte_is_skipped_counted_and_warned_about(pool: PgPool) {
    let dir = tempfile::tempdir().unwrap();
    let cache = fixture_cache(dir.path());
    let original = String::from_utf8(fixture("scryfall_oracle_cards.json")).unwrap();
    let poisoned = original.replace(
        "Lightning Bolt deals 3 damage to any target.",
        "Lightning Bolt deals 3 damage\\u0000 to any target.",
    );
    assert_ne!(poisoned, original);
    put_scryfall(
        &cache,
        "oracle_cards",
        "oracle-cards-fixture.json.gz",
        &gzip(poisoned.as_bytes()),
    );

    let (report, log) = run(&cache, &pool, false).await;

    let usable = fixture_cards();
    let summary = report.summary(Duration::from_millis(1500));
    let cards = imported(report.cards);
    let skipped = fixture_skipped() + 1;
    assert_eq!(cards.skipped.count, skipped);
    assert_eq!(cards.cards.inserted, usable - 1);
    assert_eq!(
        count(&pool, "cards").await,
        i64::try_from(usable - 1).unwrap()
    );
    assert!(log.contains("warning: cards: record"), "{log}");
    assert!(log.contains("NUL byte"), "{log}");
    assert!(summary.contains(&format!("skipped {skipped}")), "{summary}");
    let run = Database::from_pool(pool)
        .import_runs()
        .latest_succeeded(ImportSource::ScryfallCards)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(run.counts["skipped"]["count"], skipped);
}

#[sqlx::test]
async fn runs_left_running_by_a_dead_process_are_marked_failed_at_the_next_ingest(pool: PgPool) {
    let dir = tempfile::tempdir().unwrap();
    let cache = fixture_cache(dir.path());
    let db = Database::from_pool(pool.clone());
    db.migrate().await.unwrap();
    let runs = db.import_runs();
    let dead = runs.start(ImportSource::ScryfallCards).await.unwrap();
    sqlx::query("UPDATE import_runs SET started_at = now() - interval '2 hours' WHERE id = $1")
        .bind(dead.0)
        .execute(&pool)
        .await
        .unwrap();
    let alive = runs.start(ImportSource::MtgjsonDecks).await.unwrap();

    run(&cache, &pool, false).await;

    let dead = runs.get(dead).await.unwrap().unwrap();
    assert_eq!(dead.status, RunStatus::Failed);
    assert!(dead.error.unwrap().contains("abandoned"));
    assert_eq!(
        runs.get(alive).await.unwrap().unwrap().status,
        RunStatus::Running
    );
}

#[sqlx::test]
async fn a_file_cut_short_fails_its_run_and_the_rerun_finishes_the_job(pool: PgPool) {
    let dir = tempfile::tempdir().unwrap();
    let cache = fixture_cache(dir.path());
    let whole = std::fs::read(cache.path("scryfall/oracle-cards-fixture.json.gz")).unwrap();
    put_scryfall(
        &cache,
        "oracle_cards",
        "oracle-cards-fixture.json.gz",
        &whole[..whole.len() * 6 / 10],
    );
    let db = Database::from_pool(pool.clone());
    let (progress, _) = progress();

    let error = ingest(&cache, &db, &options(false), &progress)
        .await
        .unwrap_err();

    assert!(
        matches!(error, CliError::Import(ImportError::Stream { .. })),
        "{error}"
    );
    let kept = count(&pool, "cards").await;
    let usable = i64::try_from(fixture_cards()).unwrap();
    assert!(kept > 0 && kept < usable, "{kept} of {usable} cards kept");
    assert_eq!(
        kept % i64::try_from(BATCH).unwrap(),
        0,
        "only whole batches are kept"
    );
    let failed = db
        .import_runs()
        .latest(ImportSource::ScryfallCards)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(failed.status, RunStatus::Failed);
    assert_eq!(
        count(&pool, "rulings").await,
        0,
        "nothing after the failure ran"
    );

    put_scryfall(
        &cache,
        "oracle_cards",
        "oracle-cards-fixture.json.gz",
        &whole,
    );
    let (report, _) = run(&cache, &pool, false).await;

    let shape = kinds(&report);
    let cards = imported(report.cards);
    assert_eq!(i64::try_from(cards.cards.unchanged).unwrap(), kept);
    assert_eq!(i64::try_from(cards.cards.inserted).unwrap(), usable - kept);
    assert_eq!(count(&pool, "cards").await, usable);
    assert_eq!(shape, ["imported", "imported", "imported"]);
}

#[test]
fn a_dry_run_counts_what_an_import_would_do_and_needs_no_database() {
    let dir = tempfile::tempdir().unwrap();
    let cache = fixture_cache(dir.path());
    let original = String::from_utf8(fixture("scryfall_oracle_cards.json")).unwrap();
    let poisoned = original.replace(
        "Lightning Bolt deals 3 damage to any target.",
        "Lightning Bolt deals 3 damage\\u0000 to any target.",
    );
    put_scryfall(
        &cache,
        "oracle_cards",
        "oracle-cards-fixture.json.gz",
        &gzip(poisoned.as_bytes()),
    );
    let (progress, log) = progress();

    let report = dry_run(&cache, &progress).unwrap();

    let Step::Checked(cards) = &report.cards else {
        panic!("cards were not checked");
    };
    assert_eq!(
        (cards.usable, cards.skipped.count),
        (fixture_cards() - 1, fixture_skipped() + 1)
    );
    assert_eq!(kinds(&report), ["checked", "checked", "checked"]);
    let summary = report.summary(Duration::from_secs(1));
    assert!(
        summary.starts_with("ingest: dry run, nothing written: cards "),
        "{summary}"
    );
    assert!(log.text().contains("would be skipped"), "{}", log.text());
}

#[test]
fn an_empty_cache_is_refused_by_ingest_and_dry_run_alike() {
    let dir = tempfile::tempdir().unwrap();
    let cache = Cache::new(dir.path());
    let error = dry_run(&cache, &Progress::buffer().0).unwrap_err();
    assert!(error.to_string().contains("nothing is cached"), "{error}");
}

#[sqlx::test]
async fn an_empty_cache_is_refused_before_the_database_is_touched(pool: PgPool) {
    let dir = tempfile::tempdir().unwrap();
    let (progress, _) = progress();

    let error = ingest(
        &Cache::new(dir.path()),
        &Database::from_pool(pool.clone()),
        &options(false),
        &progress,
    )
    .await
    .unwrap_err();

    assert!(error.to_string().contains("nothing is cached"), "{error}");
    let tables: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM information_schema.tables WHERE table_schema = 'public'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(tables, 0, "not even the schema was created");
}
