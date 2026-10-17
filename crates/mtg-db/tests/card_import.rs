//! Goal: importing the recorded Scryfall file is idempotent at any batch size, independent of batch boundaries, and round-trips every field of every card and printing.

mod common;

use mtg_db::{ImplStatus, RunStatus};
use sqlx::PgPool;

use common::{card_stream, db, fixture_records, reset, row_counts, snapshot};

const FIXTURE_ENTRIES: usize = 24;
const FIXTURE_USABLE: u64 = 23;
const FIXTURE_ROWS: i64 = 23;

#[sqlx::test]
async fn the_fixture_pipeline_loads_every_usable_record_and_reports_the_bad_one(pool: PgPool) {
    let database = db(pool.clone());
    let report = database
        .importer()
        .import_cards(card_stream())
        .await
        .unwrap();

    assert_eq!(report.cards.inserted, FIXTURE_USABLE);
    assert_eq!(report.printings.inserted, FIXTURE_USABLE);
    assert_eq!((report.cards.updated, report.cards.unchanged), (0, 0));
    assert_eq!(report.skipped.count, 1);
    assert_eq!(report.skipped.samples[0].position, 16);
    assert!(report.skipped.samples[0].reason.contains("name"));
    assert_eq!(row_counts(&pool).await, (FIXTURE_ROWS, FIXTURE_ROWS, 0));
    assert_eq!(card_stream().count(), FIXTURE_ENTRIES);

    let run = database
        .import_runs()
        .get(report.run_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(run.status, RunStatus::Succeeded);
    assert!(run.finished_at.unwrap() >= run.started_at);
    assert_eq!(run.counts["cards"]["inserted"], FIXTURE_USABLE);
    assert_eq!(run.counts["skipped"]["count"], 1);
    assert_eq!(run.error, None);
}

#[sqlx::test]
async fn every_stored_card_equals_the_record_it_came_from(pool: PgPool) {
    let database = common::seeded(pool).await;
    for record in fixture_records() {
        let card = database
            .cards()
            .get(&record.oracle_id)
            .await
            .unwrap()
            .unwrap();
        let context = &record.name;
        assert_eq!(card.name, record.name, "{context}");
        assert_eq!(card.layout, record.layout, "{context}");
        assert_eq!(card.is_deck_card, record.layout.is_deck_card(), "{context}");
        assert_eq!(card.mana_cost, record.mana_cost, "{context}");
        assert!(
            (card.mana_value - record.mana_value).abs() < f64::EPSILON,
            "{context}"
        );
        assert_eq!(card.type_line, record.type_line, "{context}");
        assert_eq!(card.keywords, record.keywords, "{context}");
        assert_eq!(card.colors, record.colors, "{context}");
        assert_eq!(card.color_identity, record.color_identity, "{context}");
        assert_eq!(card.produced_mana, record.produced_mana, "{context}");
        assert_eq!(card.legalities, record.legalities, "{context}");
        assert_eq!(card.impl_status, ImplStatus::Manual, "{context}");
        assert_eq!(card.faces.len(), record.faces().count(), "{context}");
        for (stored, face) in card.faces.iter().zip(record.faces()) {
            assert_eq!(stored.name, face.name, "{context}");
            assert_eq!(stored.mana_cost, face.mana_cost, "{context}");
            assert_eq!(stored.type_line, face.type_line, "{context}");
            assert_eq!(stored.oracle_text, face.oracle_text, "{context}");
            assert_eq!(stored.power, face.power, "{context}");
            assert_eq!(stored.toughness, face.toughness, "{context}");
            assert_eq!(stored.loyalty, face.loyalty, "{context}");
            assert_eq!(stored.defense, face.defense, "{context}");
            assert_eq!(stored.colors, face.colors, "{context}");
        }
    }
}

#[sqlx::test]
async fn printings_keep_their_metadata_and_images_apart_from_the_card(pool: PgPool) {
    let database = common::seeded(pool).await;
    for record in fixture_records() {
        let detail = database
            .cards()
            .detail(&record.oracle_id)
            .await
            .unwrap()
            .unwrap();
        let [printing] = detail.printings.as_slice() else {
            panic!("{} should have one printing", record.name);
        };
        assert_eq!(printing.printing_id, record.printing_id);
        assert_eq!(printing.set, record.set);
        assert_eq!(printing.collector_number, record.collector_number);
        assert_eq!(printing.rarity, record.rarity);
        assert_eq!(printing.lang, record.lang);
        assert_eq!(printing.digital, record.digital);
        assert_eq!(
            printing.released_at.map(|date| date.to_string()),
            record.released_at
        );
        assert_eq!(printing.images.card, record.images);
        let face_images: Vec<_> = record.faces().map(|face| face.images.clone()).collect();
        assert_eq!(printing.images.faces, face_images);
    }
}

/// Batch sizes from one transaction per record to everything in one transaction.
const BATCH_SIZES: [usize; 6] = [1, 2, 7, 23, 24, 500];

#[sqlx::test]
async fn reimporting_changes_nothing_at_any_batch_size(pool: PgPool) {
    for batch_size in BATCH_SIZES {
        reset(&pool).await;
        let importer = db(pool.clone()).importer().with_batch_size(batch_size);
        importer.import_cards(card_stream()).await.unwrap();
        let cards = snapshot(&pool, "cards", "oracle_id", true).await;
        let printings = snapshot(&pool, "printings", "printing_id", true).await;

        let again = importer.import_cards(card_stream()).await.unwrap();

        assert_eq!(
            snapshot(&pool, "cards", "oracle_id", true).await,
            cards,
            "{batch_size}"
        );
        assert_eq!(
            snapshot(&pool, "printings", "printing_id", true).await,
            printings,
            "{batch_size}"
        );
        assert_eq!(
            again.cards.inserted + again.cards.updated,
            0,
            "{batch_size}"
        );
        assert_eq!(again.cards.unchanged, FIXTURE_USABLE, "{batch_size}");
        assert_eq!(again.printings.unchanged, FIXTURE_USABLE, "{batch_size}");
    }
}

#[sqlx::test]
async fn batch_size_does_not_change_what_is_stored(pool: PgPool) {
    let importer = db(pool.clone()).importer();
    importer
        .clone()
        .with_batch_size(500)
        .import_cards(card_stream())
        .await
        .unwrap();
    let reference = snapshot(&pool, "cards", "oracle_id", false).await;
    let reference_printings = snapshot(&pool, "printings", "printing_id", false).await;

    for batch_size in BATCH_SIZES {
        reset(&pool).await;
        importer
            .clone()
            .with_batch_size(batch_size)
            .import_cards(card_stream())
            .await
            .unwrap();
        assert_eq!(
            snapshot(&pool, "cards", "oracle_id", false).await,
            reference,
            "{batch_size}"
        );
        assert_eq!(
            snapshot(&pool, "printings", "printing_id", false).await,
            reference_printings,
            "{batch_size}"
        );
    }
}
