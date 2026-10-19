//! Goal: a run left `running` by a process that died is marked failed once it is older than the limit and at the next import start, live and finished runs stay untouched, and a run remembers the input it read so the latest successful one can be found.

mod common;

use std::time::Duration;

use mtg_db::{ImportSource, RunId, RunStatus, STALE_AFTER};
use serde_json::json;
use sqlx::PgPool;

use common::{card_stream, db};

const HOUR: Duration = Duration::from_hours(1);

/// Moves the start of a run `age` into the past.
async fn age(pool: &PgPool, run: RunId, age: Duration) {
    sqlx::query(
        "UPDATE import_runs SET started_at = now() - make_interval(secs => $2) WHERE id = $1",
    )
    .bind(run.0)
    .bind(age.as_secs_f64())
    .execute(pool)
    .await
    .unwrap_or_else(|e| panic!("aging a run failed: {e}"));
}

/// `(age of the run, limit, whether the run is reaped)`.
const LIMIT_CASES: [(Duration, Duration, bool); 5] = [
    (Duration::from_mins(59), HOUR, false),
    (Duration::from_mins(61), HOUR, true),
    (Duration::from_secs(5), Duration::from_mins(1), false),
    (Duration::from_secs(1), Duration::ZERO, true),
    (Duration::from_hours(48), Duration::from_hours(24), true),
];

#[sqlx::test]
async fn only_runs_older_than_the_limit_are_reaped(pool: PgPool) {
    let runs = db(pool.clone()).import_runs();
    for (run_age, limit, reaped) in LIMIT_CASES {
        let case = format!("age {run_age:?}, limit {limit:?}");
        let run = runs.start(ImportSource::ScryfallCards).await.unwrap();
        age(&pool, run, run_age).await;

        let ids = runs.reap_stale(limit).await.unwrap();

        assert_eq!(ids, if reaped { vec![run] } else { vec![] }, "{case}");
        let stored = runs.get(run).await.unwrap().unwrap();
        if reaped {
            assert_eq!(stored.status, RunStatus::Failed, "{case}");
            assert!(stored.finished_at.unwrap() >= stored.started_at, "{case}");
            assert!(stored.error.unwrap().contains("abandoned"), "{case}");
        } else {
            let open = (stored.status, stored.finished_at);
            assert_eq!(open, (RunStatus::Running, None), "{case}");
            runs.finish(run, &json!({})).await.unwrap();
        }
    }
}

#[sqlx::test]
async fn finished_runs_are_never_reaped_and_a_reaped_run_cannot_be_closed_again(pool: PgPool) {
    let runs = db(pool.clone()).import_runs();
    let done = runs.start(ImportSource::ScryfallRulings).await.unwrap();
    runs.finish(done, &json!({"inserted": 1})).await.unwrap();
    let failed = runs.start(ImportSource::ScryfallRulings).await.unwrap();
    runs.fail(failed, "boom", &json!({})).await.unwrap();
    let stale = runs.start(ImportSource::MtgjsonDecks).await.unwrap();
    for run in [done, failed, stale] {
        age(&pool, run, 3 * HOUR).await;
    }

    assert_eq!(runs.reap_stale(HOUR).await.unwrap(), vec![stale]);

    let done = runs.get(done).await.unwrap().unwrap();
    assert_eq!(
        (done.status, done.counts),
        (RunStatus::Succeeded, json!({"inserted": 1}))
    );
    assert_eq!(
        runs.get(failed).await.unwrap().unwrap().error.as_deref(),
        Some("boom")
    );
    assert!(runs.finish(stale, &json!({})).await.is_err());
    assert_eq!(runs.reap_stale(HOUR).await.unwrap(), vec![]);
}

#[sqlx::test]
async fn the_next_import_start_reaps_the_runs_of_dead_processes(pool: PgPool) {
    let database = db(pool.clone());
    let runs = database.import_runs();
    let dead = runs.start(ImportSource::ScryfallCards).await.unwrap();
    age(&pool, dead, STALE_AFTER + Duration::from_mins(1)).await;
    let alive = runs.start(ImportSource::MtgjsonDecks).await.unwrap();
    age(&pool, alive, Duration::from_mins(1)).await;

    let report = database
        .importer()
        .import_cards(card_stream())
        .await
        .unwrap();

    let dead = runs.get(dead).await.unwrap().unwrap();
    assert_eq!(dead.status, RunStatus::Failed);
    assert!(dead.error.unwrap().contains("abandoned"));
    assert_eq!(
        runs.get(alive).await.unwrap().unwrap().status,
        RunStatus::Running
    );
    assert_eq!(
        runs.get(report.run_id).await.unwrap().unwrap().status,
        RunStatus::Succeeded
    );
}

#[sqlx::test]
async fn a_run_remembers_its_input_and_the_latest_success_is_found(pool: PgPool) {
    let runs = db(pool).import_runs();
    let source = ImportSource::ScryfallRulings;
    assert_eq!(runs.latest_succeeded(source).await.unwrap(), None);

    let plain = runs.start(source).await.unwrap();
    assert_eq!(runs.get(plain).await.unwrap().unwrap().input, None);
    runs.finish(plain, &json!({})).await.unwrap();
    let first = runs.start_with_input(source, Some("sha-1")).await.unwrap();
    runs.finish(first, &json!({})).await.unwrap();
    let failed = runs.start_with_input(source, Some("sha-2")).await.unwrap();
    runs.fail(failed, "boom", &json!({})).await.unwrap();
    let _running = runs.start_with_input(source, Some("sha-3")).await.unwrap();
    let other = runs
        .start_with_input(ImportSource::ScryfallCards, Some("other"))
        .await
        .unwrap();
    runs.finish(other, &json!({})).await.unwrap();

    let latest = runs.latest_succeeded(source).await.unwrap().unwrap();
    assert_eq!((latest.id, latest.input.as_deref()), (first, Some("sha-1")));
}

#[sqlx::test]
async fn an_importer_records_the_input_it_was_given(pool: PgPool) {
    let database = db(pool);

    let report = database
        .importer()
        .with_input("sha-of-the-file")
        .import_cards(card_stream())
        .await
        .unwrap();

    let run = database
        .import_runs()
        .get(report.run_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(run.input.as_deref(), Some("sha-of-the-file"));
    let unnamed = database
        .importer()
        .import_cards(card_stream())
        .await
        .unwrap();
    let unnamed = database
        .import_runs()
        .get(unnamed.run_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(unnamed.input, None);
}
