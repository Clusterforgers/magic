//! Goal: importing Scryfall records is idempotent at any batch size, replaces changed card data, never touches engine coverage, and round-trips every field of the recorded fixture.

mod common;

use std::collections::BTreeMap;

use mtg_db::{CardSearch, Database, ImplStatus, Page, RunStatus};
use mtg_import::scryfall::{Legality, Rarity};
use serde_json::Value;
use sqlx::PgPool;

use common::{
    another_card, another_printing, card_stream, colors, db, fixture_records, oracle_of, record,
    row_counts, snapshot,
};

const FIXTURE_ENTRIES: usize = 24;
const FIXTURE_USABLE: u64 = 23;

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
    assert_eq!(
        row_counts(&pool).await,
        (FIXTURE_USABLE as i64, FIXTURE_USABLE as i64, 0)
    );
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
        let card = database.cards().get(&record.oracle_id).await.unwrap().unwrap();
        let context = &record.name;
        assert_eq!(card.name, record.name, "{context}");
        assert_eq!(card.layout, record.layout, "{context}");
        assert_eq!(card.is_deck_card, record.layout.is_deck_card(), "{context}");
        assert_eq!(card.mana_cost, record.mana_cost, "{context}");
        assert!((card.mana_value - record.mana_value).abs() < f64::EPSILON, "{context}");
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
        let detail = database.cards().detail(&record.oracle_id).await.unwrap().unwrap();
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

async fn reset(pool: &PgPool) {
    sqlx::raw_sql("TRUNCATE cards CASCADE")
        .execute(pool)
        .await
        .unwrap();
}

#[sqlx::test]
async fn reimporting_changes_nothing_at_any_batch_size(pool: PgPool) {
    for batch_size in BATCH_SIZES {
        reset(&pool).await;
        let importer = db(pool.clone()).importer().with_batch_size(batch_size);
        importer.import_cards(card_stream()).await.unwrap();
        let cards = snapshot(&pool, "cards", "oracle_id", true).await;
        let printings = snapshot(&pool, "printings", "printing_id", true).await;

        let again = importer.import_cards(card_stream()).await.unwrap();

        assert_eq!(snapshot(&pool, "cards", "oracle_id", true).await, cards, "{batch_size}");
        assert_eq!(
            snapshot(&pool, "printings", "printing_id", true).await,
            printings,
            "{batch_size}"
        );
        assert_eq!(again.cards.inserted + again.cards.updated, 0, "{batch_size}");
        assert_eq!(again.cards.unchanged, FIXTURE_USABLE, "{batch_size}");
        assert_eq!(again.printings.unchanged, FIXTURE_USABLE, "{batch_size}");
    }
}

#[sqlx::test]
async fn batch_size_does_not_change_what_is_stored(pool: PgPool) {
    let importer = db(pool.clone()).importer();
    importer.clone().with_batch_size(500).import_cards(card_stream()).await.unwrap();
    let reference = snapshot(&pool, "cards", "oracle_id", false).await;
    let reference_printings = snapshot(&pool, "printings", "printing_id", false).await;

    for batch_size in BATCH_SIZES {
        reset(&pool).await;
        importer.clone().with_batch_size(batch_size).import_cards(card_stream()).await.unwrap();
        assert_eq!(snapshot(&pool, "cards", "oracle_id", false).await, reference, "{batch_size}");
        assert_eq!(
            snapshot(&pool, "printings", "printing_id", false).await,
            reference_printings,
            "{batch_size}"
        );
    }
}

async fn updated_at_by_card(pool: &PgPool) -> BTreeMap<String, String> {
    let rows: Vec<(String, String)> = sqlx::query_as(
        "SELECT oracle_id::text, updated_at::text FROM cards ORDER BY oracle_id",
    )
    .fetch_all(pool)
    .await
    .unwrap();
    rows.into_iter().collect()
}

#[sqlx::test]
async fn changed_card_data_replaces_the_old_data(pool: PgPool) {
    let database = common::seeded(pool.clone()).await;
    let before = updated_at_by_card(&pool).await;
    let mut records = fixture_records();
    let bolt = records.iter_mut().find(|r| r.name == "Lightning Bolt").unwrap();
    bolt.front.oracle_text = "Lightning Bolt deals 4 damage to any target.".into();
    bolt.mana_value = 2.0;
    bolt.mana_cost = "{1}{R}".into();
    bolt.keywords.push("Haste".into());
    bolt.legalities.insert("modern".into(), Legality::Banned);
    bolt.colors = colors("RU");
    bolt.color_identity = colors("RU");
    bolt.rarity = Some(Rarity::Mythic);

    let report = database
        .importer()
        .import_cards(records.into_iter().map(Ok))
        .await
        .unwrap();

    assert_eq!((report.cards.inserted, report.cards.updated), (0, 1));
    assert_eq!(report.cards.unchanged, FIXTURE_USABLE - 1);
    assert_eq!((report.printings.updated, report.printings.unchanged), (1, FIXTURE_USABLE - 1));
    let card = database.cards().get(&oracle_of("Lightning Bolt")).await.unwrap().unwrap();
    assert_eq!(card.faces[0].oracle_text, "Lightning Bolt deals 4 damage to any target.");
    assert_eq!(card.oracle_text, "Lightning Bolt deals 4 damage to any target.");
    assert_eq!(card.mana_cost, "{1}{R}");
    assert!((card.mana_value - 2.0).abs() < f64::EPSILON);
    assert_eq!(card.keywords, ["Haste"]);
    assert_eq!(card.legalities["modern"], Legality::Banned);
    assert_eq!((card.colors, card.color_identity), (colors("RU"), colors("RU")));
    let detail = database.cards().detail(&card.oracle_id).await.unwrap().unwrap();
    assert_eq!(detail.printings[0].rarity, Some(Rarity::Mythic));

    // Only the changed card was rewritten, and the search vector followed its text.
    let after = updated_at_by_card(&pool).await;
    let changed: Vec<_> = after.iter().filter(|(id, at)| before[*id] != **at).collect();
    assert_eq!(changed.len(), 1);
    assert_eq!(changed[0].0, &card.oracle_id.to_string());
    let old_text = CardSearch { oracle_text: Some("\"3 damage\"".into()), ..CardSearch::default() };
    assert_eq!(database.cards().search(&old_text, Page::default()).await.unwrap().total, 0);
    let new_text = CardSearch { oracle_text: Some("\"4 damage\"".into()), ..CardSearch::default() };
    assert_eq!(database.cards().search(&new_text, Page::default()).await.unwrap().total, 1);
}

#[sqlx::test]
async fn engine_coverage_survives_reimports_of_changed_cards(pool: PgPool) {
    let database = common::seeded(pool.clone()).await;
    sqlx::raw_sql(
        "UPDATE cards SET impl_status = 'full' WHERE name = 'Shock';
         UPDATE cards SET impl_status = 'partial' WHERE name = 'Lightning Bolt'",
    )
    .execute(&pool)
    .await
    .unwrap();
    let mut records = fixture_records();
    for card in records.iter_mut().filter(|r| r.name == "Shock") {
        card.front.oracle_text = "Shock deals 3 damage to any target.".into();
    }

    database.importer().import_cards(records.into_iter().map(Ok)).await.unwrap();

    let status = |name: &'static str| {
        let database = database.clone();
        async move {
            database.cards().get(&oracle_of(name)).await.unwrap().unwrap().impl_status
        }
    };
    assert_eq!(status("Shock").await, ImplStatus::Full);
    assert_eq!(status("Lightning Bolt").await, ImplStatus::Partial);
    assert_eq!(status("Sol Ring").await, ImplStatus::Manual);
    let shock = database.cards().get(&oracle_of("Shock")).await.unwrap().unwrap();
    assert_eq!(shock.oracle_text, "Shock deals 3 damage to any target.");
}

#[sqlx::test]
async fn many_printings_of_one_card_share_a_single_card_row(pool: PgPool) {
    let bolt = record("Lightning Bolt");
    let records = vec![
        bolt.clone(),
        record("Shock"),
        another_printing(&bolt, 2, "lea", Some("1993-08-05")),
        another_printing(&bolt, 3, "m11", Some("2010-07-16")),
        another_printing(&bolt, 4, "xxx", None),
    ];

    // (batch size, cards inserted, cards offered again unchanged): a card is counted per batch.
    for (batch_size, inserted, unchanged) in [(500, 2, 0), (1, 2, 3)] {
        reset(&pool).await;
        let report = db(pool.clone())
            .importer()
            .with_batch_size(batch_size)
            .import_cards(records.clone().into_iter().map(Ok))
            .await
            .unwrap();

        assert_eq!(row_counts(&pool).await, (2, 5, 0), "{batch_size}");
        assert_eq!(report.printings.inserted, 5, "{batch_size}");
        assert_eq!(report.cards.inserted, inserted, "{batch_size}");
        assert_eq!(report.cards.unchanged, unchanged, "{batch_size}");
        let detail = db(pool.clone()).cards().detail(&bolt.oracle_id).await.unwrap().unwrap();
        let sets: Vec<_> = detail.printings.iter().map(|p| p.set.as_str()).collect();
        assert_eq!(sets, ["m11", "m10", "lea", "xxx"], "newest first, undated last");
    }
}

#[sqlx::test]
async fn the_last_record_of_a_batch_wins_for_shared_card_data(pool: PgPool) {
    let bolt = record("Lightning Bolt");
    let mut newer = another_printing(&bolt, 2, "m11", Some("2010-07-16"));
    newer.front.oracle_text = "Corrected text.".into();
    let database = db(pool);

    database
        .importer()
        .import_cards([bolt.clone(), newer].into_iter().map(Ok))
        .await
        .unwrap();

    let card = database.cards().get(&bolt.oracle_id).await.unwrap().unwrap();
    assert_eq!(card.oracle_text, "Corrected text.");
}

#[sqlx::test]
async fn other_cards_are_untouched_by_an_empty_or_tiny_import(pool: PgPool) {
    let database = common::seeded(pool.clone()).await;
    let before = snapshot(&pool, "cards", "oracle_id", true).await;

    let empty = database.importer().import_cards(Vec::new()).await.unwrap();
    let extra = another_card(&record("Shock"), 77, "Brand New Card");
    let one = database.importer().import_cards([Ok(extra)]).await.unwrap();

    assert_eq!(empty.cards, Default::default());
    assert_eq!(one.cards.inserted, 1);
    let after = snapshot(&pool, "cards", "oracle_id", true).await;
    let (Value::Array(before), Value::Array(after)) = (before, after) else {
        panic!("snapshots are arrays");
    };
    assert_eq!(after.len(), before.len() + 1);
    assert!(before.iter().all(|row| after.contains(row)));
}

#[sqlx::test]
async fn jsonl_bulk_files_import_like_array_files(pool: PgPool) {
    let array = std::fs::read_to_string(common::fixture_path("scryfall_oracle_cards.json")).unwrap();
    let entries: Vec<Value> = serde_json::from_str(&array).unwrap();
    let lines: Vec<String> = entries.iter().map(Value::to_string).collect();
    let jsonl = lines.join("\n");
    let database = Database::from_pool(pool.clone());

    let report = database
        .importer()
        .import_cards(mtg_import::scryfall::cards_jsonl(jsonl.as_bytes()))
        .await
        .unwrap();

    assert_eq!(report.cards.inserted, FIXTURE_USABLE);
    assert_eq!(report.skipped.count, 1);
}
