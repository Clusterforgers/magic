//! Goal: rulings ingest is idempotent, keeps stream order, attaches only to stored cards, and reports what it skipped or could not place.

use crate::common;

use mtg_db::RunStatus;
use mtg_import::scryfall::{RulingRecord, RulingSource, rulings};
use sqlx::PgPool;

use common::{db, oracle_of, row_counts, ruling_stream, seeded, snapshot};

fn ruling(card: &str, date: Option<&str>, comment: &str) -> RulingRecord {
    RulingRecord {
        oracle_id: oracle_of(card),
        source: RulingSource::Wotc,
        published_at: date.map(str::to_owned),
        comment: comment.to_owned(),
    }
}

#[sqlx::test]
async fn the_fixture_pipeline_attaches_rulings_to_their_cards(pool: PgPool) {
    let database = seeded(pool.clone()).await;

    let report = database
        .importer()
        .import_rulings(ruling_stream())
        .await
        .unwrap();

    assert_eq!(
        (report.inserted, report.unchanged, report.orphaned),
        (5, 0, 0)
    );
    assert_eq!(report.skipped.count, 2);
    let positions: Vec<u64> = report.skipped.samples.iter().map(|s| s.position).collect();
    assert_eq!(positions, [5, 6]);
    assert_eq!(row_counts(&pool).await.2, 5);

    let detail = database
        .cards()
        .detail(&oracle_of("Fire // Ice"))
        .await
        .unwrap()
        .unwrap();
    let sources: Vec<_> = detail.rulings.iter().map(|r| r.source).collect();
    assert_eq!(sources, [RulingSource::Wotc, RulingSource::Scryfall]);
    assert_eq!(
        detail.rulings[0]
            .published_at
            .map(|d| d.to_string())
            .as_deref(),
        Some("2004-10-04")
    );
    assert!(
        detail.rulings[1].comment.contains('—'),
        "non-ASCII text survives"
    );
    let bolt = database
        .cards()
        .detail(&oracle_of("Lightning Bolt"))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(bolt.rulings[0].source, RulingSource::Unknown);

    let run = database
        .import_runs()
        .get(report.run_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(run.status, RunStatus::Succeeded);
    assert_eq!(run.counts["inserted"], 5);
}

#[sqlx::test]
async fn reimporting_rulings_changes_nothing(pool: PgPool) {
    let database = seeded(pool.clone()).await;
    database
        .importer()
        .import_rulings(ruling_stream())
        .await
        .unwrap();
    let before = snapshot(&pool, "rulings", "id", true).await;

    for batch_size in [1, 2, 500] {
        let again = database
            .importer()
            .with_batch_size(batch_size)
            .import_rulings(ruling_stream())
            .await
            .unwrap();
        assert_eq!((again.inserted, again.unchanged), (0, 5), "{batch_size}");
        assert_eq!(
            snapshot(&pool, "rulings", "id", true).await,
            before,
            "{batch_size}"
        );
    }
}

#[sqlx::test]
async fn rulings_for_unknown_cards_are_counted_not_stored(pool: PgPool) {
    let database = db(pool.clone());

    let early = database
        .importer()
        .import_rulings(ruling_stream())
        .await
        .unwrap();
    assert_eq!((early.inserted, early.orphaned), (0, 5));
    assert_eq!(row_counts(&pool).await.2, 0);

    database
        .importer()
        .import_cards(common::card_stream())
        .await
        .unwrap();
    let late = database
        .importer()
        .import_rulings(ruling_stream())
        .await
        .unwrap();
    assert_eq!((late.inserted, late.orphaned), (5, 0));
}

#[sqlx::test]
async fn identical_rulings_collapse_and_near_identical_ones_do_not(pool: PgPool) {
    let database = seeded(pool.clone()).await;
    let records = vec![
        ruling("Shock", Some("2020-01-01"), "Same text."),
        ruling("Shock", Some("2020-01-01"), "Same text."),
        ruling("Shock", Some("2020-01-02"), "Same text."),
        ruling("Shock", None, "Same text."),
        ruling("Shock", None, "Same text."),
        ruling("Sol Ring", Some("2020-01-01"), "Same text."),
        ruling("Shock", Some("2020-01-01"), "Same text!"),
    ];
    let mut other_source = ruling("Shock", Some("2020-01-01"), "Same text.");
    other_source.source = RulingSource::Scryfall;

    let report = database
        .importer()
        .import_rulings(records.into_iter().chain([other_source]).map(Ok))
        .await
        .unwrap();

    assert_eq!((report.inserted, report.unchanged), (6, 2));
    let detail = database
        .cards()
        .detail(&oracle_of("Shock"))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(detail.rulings.len(), 5);
}

#[sqlx::test]
async fn rulings_come_back_oldest_first_with_undated_last_and_ties_in_stream_order(pool: PgPool) {
    let database = seeded(pool).await;
    let records = vec![
        ruling("Shock", None, "undated"),
        ruling("Shock", Some("2022-05-05"), "third"),
        ruling("Shock", Some("2001-01-01"), "first"),
        ruling("Shock", Some("2022-05-05"), "fourth, same day"),
        ruling("Shock", Some("2010-10-10"), "second"),
    ];

    database
        .importer()
        .import_rulings(records.into_iter().map(Ok))
        .await
        .unwrap();

    let detail = database
        .cards()
        .detail(&oracle_of("Shock"))
        .await
        .unwrap()
        .unwrap();
    let comments: Vec<_> = detail.rulings.iter().map(|r| r.comment.as_str()).collect();
    assert_eq!(
        comments,
        ["first", "second", "third", "fourth, same day", "undated"]
    );
}

#[sqlx::test]
async fn unusable_rulings_are_skipped(pool: PgPool) {
    let database = seeded(pool.clone()).await;
    let mut bad_id = ruling("Shock", None, "x");
    bad_id.oracle_id.0 = "nope".into();
    let blank = ruling("Shock", None, "  \n");
    let bad_date = ruling("Shock", Some("2020-13-01"), "x");
    let good = ruling("Shock", Some("2020-01-01"), "fine");

    let report = database
        .importer()
        .import_rulings([bad_id, blank, bad_date, good].into_iter().map(Ok))
        .await
        .unwrap();

    assert_eq!(report.inserted, 1);
    assert_eq!(report.skipped.count, 3);
    assert_eq!(row_counts(&pool).await.2, 1);
}

#[sqlx::test]
async fn a_fatal_stream_error_fails_the_run_after_committing_earlier_batches(pool: PgPool) {
    let database = seeded(pool.clone()).await;
    let entry = |n: u32| {
        format!(
            r#"{{"oracle_id":"{}","source":"wotc","published_at":"2020-01-0{n}","comment":"ruling {n}"}}"#,
            oracle_of("Shock")
        )
    };
    let json = format!("[{},{},{},{{\"oracle_id\":", entry(1), entry(2), entry(3));

    let error = database
        .importer()
        .with_batch_size(2)
        .import_rulings(rulings(json.as_bytes()))
        .await
        .unwrap_err();

    let mtg_db::ImportError::Stream { run_id, .. } = error else {
        panic!("expected a stream error, got {error}");
    };
    assert_eq!(row_counts(&pool).await.2, 2);
    let run = database.import_runs().get(run_id).await.unwrap().unwrap();
    assert_eq!(run.status, RunStatus::Failed);
    assert_eq!(run.counts["inserted"], 2);
}
