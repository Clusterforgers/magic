//! Goal: re-importing changed records replaces the old card data and search vectors, never touches engine coverage, folds many printings of one card into one row, and reads JSONL bulk files like array files.

mod common;

use std::collections::BTreeMap;

use mtg_db::{CardSearch, Database, ImplStatus, Page};
use mtg_import::scryfall::{Legality, Rarity};
use serde_json::Value;
use sqlx::PgPool;

use common::{
    another_card, another_printing, colors, db, fixture_records, oracle_of, record, reset,
    row_counts, snapshot,
};

const FIXTURE_USABLE: u64 = 23;

async fn updated_at_by_card(pool: &PgPool) -> BTreeMap<String, String> {
    let rows: Vec<(String, String)> =
        sqlx::query_as("SELECT oracle_id::text, updated_at::text FROM cards ORDER BY oracle_id")
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
    let bolt = records
        .iter_mut()
        .find(|r| r.name == "Lightning Bolt")
        .unwrap();
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
    assert_eq!(
        (report.printings.updated, report.printings.unchanged),
        (1, FIXTURE_USABLE - 1)
    );
    let card = database
        .cards()
        .get(&oracle_of("Lightning Bolt"))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        card.faces[0].oracle_text,
        "Lightning Bolt deals 4 damage to any target."
    );
    assert_eq!(
        card.oracle_text,
        "Lightning Bolt deals 4 damage to any target."
    );
    assert_eq!(card.mana_cost, "{1}{R}");
    assert!((card.mana_value - 2.0).abs() < f64::EPSILON);
    assert_eq!(card.keywords, ["Haste"]);
    assert_eq!(card.legalities["modern"], Legality::Banned);
    assert_eq!(
        (card.colors, card.color_identity),
        (colors("RU"), colors("RU"))
    );
    let detail = database
        .cards()
        .detail(&card.oracle_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(detail.printings[0].rarity, Some(Rarity::Mythic));

    // Only the changed card was rewritten, and the search vector followed its text.
    let after = updated_at_by_card(&pool).await;
    let changed: Vec<_> = after
        .iter()
        .filter(|(id, at)| before[*id] != **at)
        .collect();
    assert_eq!(changed.len(), 1);
    assert_eq!(changed[0].0, &card.oracle_id.to_string());
    let old_text = CardSearch {
        oracle_text: Some("\"3 damage\"".into()),
        ..CardSearch::default()
    };
    assert_eq!(
        database
            .cards()
            .search(&old_text, Page::default())
            .await
            .unwrap()
            .total,
        0
    );
    let new_text = CardSearch {
        oracle_text: Some("\"4 damage\"".into()),
        ..CardSearch::default()
    };
    assert_eq!(
        database
            .cards()
            .search(&new_text, Page::default())
            .await
            .unwrap()
            .total,
        1
    );
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

    database
        .importer()
        .import_cards(records.into_iter().map(Ok))
        .await
        .unwrap();

    let status = |name: &'static str| {
        let database = database.clone();
        async move {
            database
                .cards()
                .get(&oracle_of(name))
                .await
                .unwrap()
                .unwrap()
                .impl_status
        }
    };
    assert_eq!(status("Shock").await, ImplStatus::Full);
    assert_eq!(status("Lightning Bolt").await, ImplStatus::Partial);
    assert_eq!(status("Sol Ring").await, ImplStatus::Manual);
    let shock = database
        .cards()
        .get(&oracle_of("Shock"))
        .await
        .unwrap()
        .unwrap();
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
        let detail = db(pool.clone())
            .cards()
            .detail(&bolt.oracle_id)
            .await
            .unwrap()
            .unwrap();
        let sets: Vec<_> = detail.printings.iter().map(|p| p.set.as_str()).collect();
        assert_eq!(
            sets,
            ["m11", "m10", "lea", "xxx"],
            "newest first, undated last"
        );
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

    let card = database
        .cards()
        .get(&bolt.oracle_id)
        .await
        .unwrap()
        .unwrap();
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
    let array =
        std::fs::read_to_string(common::fixture_path("scryfall_oracle_cards.json")).unwrap();
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
