//! Goal: import-run bookkeeping moves each run through running to exactly one terminal state, rejects invalid transitions, and lists history newest first.

mod common;

use mtg_db::{DbError, ImportSource, RunId, RunStatus};
use serde_json::json;
use sqlx::PgPool;

use common::db;

#[sqlx::test]
async fn a_new_run_is_running_without_an_end(pool: PgPool) {
    let runs = db(pool).import_runs();
    let id = runs.start(ImportSource::ScryfallCards).await.unwrap();

    let run = runs.get(id).await.unwrap().unwrap();

    assert_eq!(run.id, id);
    assert_eq!(run.source, ImportSource::ScryfallCards);
    assert_eq!(run.status, RunStatus::Running);
    assert_eq!(run.finished_at, None);
    assert_eq!(run.error, None);
    assert_eq!(run.counts, json!({}));
}

#[sqlx::test]
async fn finishing_stores_the_counters_and_the_end_time(pool: PgPool) {
    let runs = db(pool).import_runs();
    let id = runs.start(ImportSource::MtgjsonDecks).await.unwrap();

    runs.finish(id, &json!({"decks": 3})).await.unwrap();

    let run = runs.get(id).await.unwrap().unwrap();
    assert_eq!(run.status, RunStatus::Succeeded);
    assert_eq!(run.counts, json!({"decks": 3}));
    assert!(run.finished_at.unwrap() >= run.started_at);
    assert_eq!(run.error, None);
}

#[sqlx::test]
async fn failing_stores_the_message_and_the_progress_so_far(pool: PgPool) {
    let runs = db(pool).import_runs();
    let id = runs.start(ImportSource::ScryfallRulings).await.unwrap();

    runs.fail(id, "bulk file truncated", &json!({"inserted": 12})).await.unwrap();

    let run = runs.get(id).await.unwrap().unwrap();
    assert_eq!(run.status, RunStatus::Failed);
    assert_eq!(run.error.as_deref(), Some("bulk file truncated"));
    assert_eq!(run.counts, json!({"inserted": 12}));
    assert!(run.finished_at.is_some());
}

#[sqlx::test]
async fn a_run_ends_only_once(pool: PgPool) {
    let runs = db(pool).import_runs();
    let finished = runs.start(ImportSource::ScryfallCards).await.unwrap();
    runs.finish(finished, &json!({})).await.unwrap();
    let failed = runs.start(ImportSource::ScryfallCards).await.unwrap();
    runs.fail(failed, "boom", &json!({})).await.unwrap();

    let attempts = [
        runs.finish(finished, &json!({"again": true})).await,
        runs.fail(finished, "late", &json!({})).await,
        runs.finish(failed, &json!({})).await,
        runs.fail(failed, "again", &json!({})).await,
        runs.finish(RunId(987_654), &json!({})).await,
    ];

    for attempt in attempts {
        assert!(matches!(attempt, Err(DbError::RunNotRunning(_))), "{attempt:?}");
    }
    let run = runs.get(finished).await.unwrap().unwrap();
    assert_eq!((run.status, run.counts), (RunStatus::Succeeded, json!({})));
    assert_eq!(runs.get(failed).await.unwrap().unwrap().error.as_deref(), Some("boom"));
}

#[sqlx::test]
async fn unknown_runs_are_absent(pool: PgPool) {
    let runs = db(pool).import_runs();
    assert_eq!(runs.get(RunId(1)).await.unwrap(), None);
    assert_eq!(runs.latest(ImportSource::ScryfallCards).await.unwrap(), None);
    assert!(runs.recent(10).await.unwrap().is_empty());
}

#[sqlx::test]
async fn history_lists_newest_first_per_source_and_overall(pool: PgPool) {
    let runs = db(pool).import_runs();
    let mut started = Vec::new();
    for source in [
        ImportSource::ScryfallCards,
        ImportSource::ScryfallRulings,
        ImportSource::ScryfallCards,
        ImportSource::MtgjsonDecks,
        ImportSource::ScryfallCards,
    ] {
        started.push((runs.start(source).await.unwrap(), source));
    }

    let recent: Vec<RunId> = runs.recent(100).await.unwrap().iter().map(|r| r.id).collect();
    let expected: Vec<RunId> = started.iter().rev().map(|(id, _)| *id).collect();
    assert_eq!(recent, expected);
    assert_eq!(runs.recent(2).await.unwrap().len(), 2);
    assert_eq!(runs.recent(0).await.unwrap().len(), 0);

    let latest_cards = runs.latest(ImportSource::ScryfallCards).await.unwrap().unwrap();
    assert_eq!(latest_cards.id, started[4].0);
    let latest_rulings = runs.latest(ImportSource::ScryfallRulings).await.unwrap().unwrap();
    assert_eq!(latest_rulings.id, started[1].0);
    let latest_decks = runs.latest(ImportSource::MtgjsonDecks).await.unwrap().unwrap();
    assert_eq!(latest_decks.id, started[3].0);
}
