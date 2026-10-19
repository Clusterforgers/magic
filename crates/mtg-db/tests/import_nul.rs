//! Goal: a record holding a NUL byte, which Postgres cannot store, is skipped and counted with a reason that never carries the byte, while every other record of its batch is stored and the run succeeds.

mod common;

use mtg_db::RunStatus;
use mtg_import::mtgjson::parse_deck_str;
use mtg_import::scryfall::{cards, rulings};
use sqlx::PgPool;

use common::precon::{deck, entry};
use common::{
    db, oracle_of, record, reset, row_counts, seeded, snapshot, synthetic_card_json,
    synthetic_oracle,
};

/// `synthetic_card_json(n)` with a NUL byte (as the JSON escape) added to its rules text.
fn with_nul(n: u128) -> String {
    synthetic_card_json(n).replace(r#""type_line""#, r#""oracle_text":"a\u0000b","type_line""#)
}

#[sqlx::test]
async fn a_card_with_a_nul_byte_is_skipped_and_the_rest_of_the_batch_is_stored(pool: PgPool) {
    let json = format!(
        "[{},{},{},{}]",
        synthetic_card_json(1),
        with_nul(2),
        synthetic_card_json(3),
        with_nul(4)
    );
    let database = db(pool.clone());
    for batch_size in [1, 3, 500] {
        reset(&pool).await;

        let report = database
            .importer()
            .with_batch_size(batch_size)
            .import_cards(cards(json.as_bytes()))
            .await
            .unwrap();

        let case = format!("batch size {batch_size}");
        assert_eq!(report.skipped.count, 2, "{case}");
        let positions: Vec<u64> = report.skipped.samples.iter().map(|s| s.position).collect();
        assert_eq!(positions, [1, 3], "{case}");
        assert!(report.skipped.samples[0].reason.contains("NUL"), "{case}");
        assert_eq!(
            (report.cards.inserted, report.printings.inserted),
            (2, 2),
            "{case}"
        );
        assert_eq!(row_counts(&pool).await, (2, 2, 0), "{case}");
        for stored in [1, 3] {
            let card = database.cards().get(&synthetic_oracle(stored)).await;
            assert!(card.unwrap().is_some(), "{case}");
        }
        let run = database
            .import_runs()
            .get(report.run_id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(run.status, RunStatus::Succeeded, "{case}");
        assert_eq!(run.counts["skipped"]["count"], 2, "{case}");
    }
}

#[sqlx::test]
async fn a_nul_byte_does_not_touch_the_stored_rows_of_other_cards(pool: PgPool) {
    let database = seeded(pool.clone()).await;
    let before = snapshot(&pool, "cards", "oracle_id", false).await;
    let mut bolt = record("Lightning Bolt");
    bolt.front.oracle_text.push('\0');

    let report = database.importer().import_cards([Ok(bolt)]).await.unwrap();

    assert_eq!((report.skipped.count, report.cards.inserted), (1, 0));
    assert_eq!(snapshot(&pool, "cards", "oracle_id", false).await, before);
}

#[sqlx::test]
async fn a_ruling_with_a_nul_byte_is_skipped_and_counted(pool: PgPool) {
    let database = seeded(pool.clone()).await;
    let oracle = oracle_of("Lightning Bolt");
    let ruling = |comment: &str| {
        format!(
            r#"{{"oracle_id":"{oracle}","source":"wotc","published_at":"2020-01-01","comment":"{comment}"}}"#
        )
    };
    let json = format!(
        "[{},{},{}]",
        ruling("Fine."),
        ruling(r"Bad\u0000."),
        ruling("Also fine.")
    );

    let report = database
        .importer()
        .import_rulings(rulings(json.as_bytes()))
        .await
        .unwrap();

    assert_eq!((report.inserted, report.skipped.count), (2, 1));
    assert_eq!(report.skipped.samples[0].position, 1);
    assert_eq!(row_counts(&pool).await.2, 2);
}

#[sqlx::test]
async fn a_deck_with_a_nul_byte_is_skipped_and_the_other_decks_are_stored(pool: PgPool) {
    let database = seeded(pool).await;
    let decks = vec![
        Ok(deck("Fine", "AAA", None, vec![entry("Lightning Bolt", 1)])),
        Ok(deck(
            "Bad name\0",
            "BBB",
            None,
            vec![entry("Lightning Bolt", 1)],
        )),
        Ok(deck(
            "Bad card",
            "CCC",
            None,
            vec![entry("Lightning\0Bolt", 1)],
        )),
        parse_deck_str("{\"name\":\"Bad code\",\"code\":\"D\\u0000D\"}"),
        Ok(deck("Also fine", "EEE", None, vec![entry("Sol Ring", 1)])),
    ];

    let report = database.importer().import_precons(decks).await.unwrap();

    let names: Vec<&str> = report.decks.iter().map(|d| d.name.as_str()).collect();
    assert_eq!(names, ["Fine", "Also fine"]);
    assert_eq!(report.skipped.count, 3);
    let positions: Vec<u64> = report.skipped.samples.iter().map(|s| s.position).collect();
    assert_eq!(positions, [1, 2, 3]);
    assert!(
        report
            .skipped
            .samples
            .iter()
            .all(|s| !s.reason.contains('\0'))
    );
    let run = database
        .import_runs()
        .get(report.run_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(run.status, RunStatus::Succeeded);
    assert_eq!(run.counts["skipped"]["count"], 3);
}

#[sqlx::test]
async fn text_echoed_into_a_reason_cannot_break_the_run_record(pool: PgPool) {
    // The parse error quotes the card name, NUL included.
    let echoing = parse_deck_str(
        "{\"name\":\"D\",\"code\":\"C\",\"mainBoard\":[{\"count\":0,\"uuid\":\"u\",\"name\":\"A\\u0000\"}]}",
    );
    assert!(echoing.as_ref().unwrap_err().to_string().contains('\0'));
    let database = db(pool);

    let report = database.importer().import_precons([echoing]).await.unwrap();

    assert_eq!(report.skipped.count, 1);
    assert!(!report.skipped.samples[0].reason.contains('\0'));
    let run = database
        .import_runs()
        .get(report.run_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(run.status, RunStatus::Succeeded);
    let reason = run.counts["skipped"]["samples"][0]["reason"].as_str();
    assert!(reason.unwrap().contains('\u{2400}'), "{reason:?}");
}
