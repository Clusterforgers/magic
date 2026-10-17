//! Goal: a bad record is counted and skipped without stopping an import, while fatal stream or database errors abort it, mark the run failed and keep exactly the batches that were already committed.

mod common;

use mtg_db::{ImportError, RunStatus};
use mtg_import::ids::OracleId;
use mtg_import::scryfall::{CardRecord, ScryfallError, cards};
use sqlx::PgPool;

use common::{db, record, row_counts, synthetic_card_json, synthetic_oracle};

fn array(entries: &[String]) -> String {
    format!("[{}]", entries.join(","))
}

/// A stream with `good` valid cards followed by a truncated entry, which is fatal.
fn truncated_after(good: u128) -> String {
    let entries: Vec<String> = (1..=good).map(synthetic_card_json).collect();
    format!(
        "[{}{}{{\"id\":",
        entries.join(","),
        if good > 0 { "," } else { "" }
    )
}

#[sqlx::test]
async fn unusable_records_are_skipped_with_their_stream_positions(pool: PgPool) {
    let json = array(&[
        synthetic_card_json(1),
        r#"{"id":"x"}"#.to_owned(),
        r#"{"id":"y","oracle_id":"o","cmc":"nope","name":"N"}"#.to_owned(),
        synthetic_card_json(2),
    ]);
    let mut items: Vec<Result<CardRecord, ScryfallError>> = cards(json.as_bytes()).collect();
    let bolt = record("Lightning Bolt");
    let mut bad_oracle_id = bolt.clone();
    bad_oracle_id.oracle_id = OracleId("not-a-uuid".into());
    let mut bad_date = bolt.clone();
    bad_date.released_at = Some("2009-02-30".into());
    let mut bad_mana_value = bolt.clone();
    bad_mana_value.mana_value = -1.0;
    items.extend(
        [bad_oracle_id, bad_date, bad_mana_value]
            .into_iter()
            .map(Ok),
    );
    items.push(Ok(common::another_card(&bolt, 3, "Fine")));

    let database = db(pool.clone());
    let report = database
        .importer()
        .with_batch_size(2)
        .import_cards(items)
        .await
        .unwrap();

    assert_eq!(report.skipped.count, 5);
    let positions: Vec<u64> = report.skipped.samples.iter().map(|s| s.position).collect();
    assert_eq!(positions, [1, 2, 4, 5, 6]);
    assert!(report.skipped.samples[2].reason.contains("not-a-uuid"));
    assert!(report.skipped.samples[3].reason.contains("2009-02-30"));
    assert!(report.skipped.samples[4].reason.contains("-1"));
    assert_eq!((report.cards.inserted, report.printings.inserted), (3, 3));
    assert_eq!(row_counts(&pool).await, (3, 3, 0));
    for good in [synthetic_oracle(1), synthetic_oracle(2)] {
        assert!(database.cards().get(&good).await.unwrap().is_some());
    }
    let run = database
        .import_runs()
        .get(report.run_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(run.status, RunStatus::Succeeded);
    assert_eq!(run.counts["skipped"]["count"], 5);
    assert_eq!(run.counts["skipped"]["samples"][0]["position"], 1);
}

#[sqlx::test]
async fn skipped_records_beyond_the_sample_limit_are_still_counted(pool: PgPool) {
    let bad = (0..45).map(|n| format!(r#"{{"id":"x{n}"}}"#));
    let json = array(&bad.collect::<Vec<_>>());

    let report = db(pool)
        .importer()
        .import_cards(cards(json.as_bytes()))
        .await
        .unwrap();

    assert_eq!(report.skipped.count, 45);
    assert_eq!(report.skipped.samples.len(), 20);
    assert_eq!(report.cards.inserted, 0);
}

/// `(batch size, good cards before the fatal error)`; the partial batch being read is dropped.
const FATAL_CASES: [(usize, u128); 8] = [
    (1, 5),
    (2, 5),
    (3, 5),
    (5, 5),
    (10, 5),
    (2, 0),
    (2, 1),
    (500, 3),
];

#[sqlx::test]
async fn a_fatal_stream_error_fails_the_run_and_keeps_only_complete_batches(pool: PgPool) {
    for (batch_size, good) in FATAL_CASES {
        sqlx::raw_sql("TRUNCATE cards CASCADE")
            .execute(&pool)
            .await
            .unwrap();
        let committed = (good / batch_size as u128) * batch_size as u128;
        let database = db(pool.clone());
        let json = truncated_after(good);

        let error = database
            .importer()
            .with_batch_size(batch_size)
            .import_cards(cards(json.as_bytes()))
            .await
            .unwrap_err();

        let case = format!("batch {batch_size}, {good} good");
        let ImportError::Stream { run_id, source } = error else {
            panic!("{case}: expected a stream error, got {error}");
        };
        assert!(source.is_fatal(), "{case}");
        assert_eq!(
            row_counts(&pool).await.0,
            i64::try_from(committed).unwrap(),
            "{case}"
        );
        let run = database.import_runs().get(run_id).await.unwrap().unwrap();
        assert_eq!(run.status, RunStatus::Failed, "{case}");
        assert!(run.finished_at.is_some(), "{case}");
        assert!(
            run.error.as_deref().unwrap().contains("malformed"),
            "{case}"
        );
        assert_eq!(
            run.counts["cards"]["inserted"],
            u64::try_from(committed).unwrap(),
            "{case}"
        );
    }
}

#[sqlx::test]
async fn a_failing_database_write_rolls_back_its_batch_but_not_earlier_ones(pool: PgPool) {
    sqlx::raw_sql("ALTER TABLE cards ADD CONSTRAINT poisoned CHECK (name <> 'Synthetic 3')")
        .execute(&pool)
        .await
        .unwrap();
    let entries: Vec<String> = (1..=6).map(synthetic_card_json).collect();
    let database = db(pool.clone());

    let error = database
        .importer()
        .with_batch_size(2)
        .import_cards(cards(array(&entries).as_bytes()))
        .await
        .unwrap_err();

    let ImportError::Database {
        run_id: Some(run_id),
        source,
    } = error
    else {
        panic!("expected a database error, got {error}");
    };
    assert!(source.to_string().contains("poisoned"), "{source}");
    // Batch one (cards 1 and 2) committed; batch two (3 and 4) rolled back as a whole.
    assert_eq!(row_counts(&pool).await, (2, 2, 0));
    assert!(
        database
            .cards()
            .get(&synthetic_oracle(4))
            .await
            .unwrap()
            .is_none()
    );
    let run = database.import_runs().get(run_id).await.unwrap().unwrap();
    assert_eq!(run.status, RunStatus::Failed);
    assert_eq!(run.counts["cards"]["inserted"], 2);

    // Fixing the cause and importing again completes the job.
    sqlx::raw_sql("ALTER TABLE cards DROP CONSTRAINT poisoned")
        .execute(&pool)
        .await
        .unwrap();
    let retry = database
        .importer()
        .with_batch_size(2)
        .import_cards(cards(array(&entries).as_bytes()))
        .await
        .unwrap();
    assert_eq!((retry.cards.inserted, retry.cards.unchanged), (4, 2));
    assert_eq!(row_counts(&pool).await.0, 6);
}

#[sqlx::test]
async fn a_missing_table_fails_the_run_without_partial_rows(pool: PgPool) {
    sqlx::raw_sql("DROP TABLE printings")
        .execute(&pool)
        .await
        .unwrap();
    let database = db(pool.clone());

    let error = database
        .importer()
        .import_cards(common::card_stream())
        .await
        .unwrap_err();

    let ImportError::Database {
        run_id: Some(run_id),
        ..
    } = error
    else {
        panic!("expected a database error, got {error}");
    };
    let cards: i64 = sqlx::query_scalar("SELECT count(*) FROM cards")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(cards, 0, "cards and printings share one transaction");
    let run = database.import_runs().get(run_id).await.unwrap().unwrap();
    assert_eq!(run.status, RunStatus::Failed);
    assert!(run.error.unwrap().contains("printings"));
}

#[sqlx::test]
async fn an_unreachable_run_table_surfaces_as_an_error_without_a_run_id(pool: PgPool) {
    sqlx::raw_sql("DROP TABLE import_runs")
        .execute(&pool)
        .await
        .unwrap();

    let error = db(pool)
        .importer()
        .import_cards(common::card_stream())
        .await
        .unwrap_err();

    assert!(
        matches!(error, ImportError::Database { run_id: None, .. }),
        "{error}"
    );
}
