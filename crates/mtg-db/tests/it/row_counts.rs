//! Goal: the row counts report every card-data table exactly, from an empty schema through cards, rulings and precon decks.

use crate::common;

use mtg_db::RowCounts;
use sqlx::PgPool;

use common::precon::ingest;
use common::{db, fixture_records, precon_deck, row_counts, ruling_stream, seeded};

#[sqlx::test]
async fn an_empty_schema_counts_zero_everywhere(pool: PgPool) {
    let counts = db(pool).row_counts().await.unwrap();

    assert_eq!(
        counts,
        RowCounts {
            cards: 0,
            printings: 0,
            rulings: 0,
            precon_decks: 0,
            precon_entries: 0
        }
    );
}

#[sqlx::test]
async fn counts_follow_each_import(pool: PgPool) {
    let database = seeded(pool.clone()).await;
    database
        .importer()
        .import_rulings(ruling_stream())
        .await
        .unwrap();
    ingest(&database, vec![precon_deck()]).await;

    let counts = database.row_counts().await.unwrap();

    let (cards, printings, rulings) = row_counts(&pool).await;
    assert_eq!(counts.cards, fixture_records().len() as u64);
    assert_eq!(
        (counts.cards, counts.printings, counts.rulings),
        (
            u64::try_from(cards).unwrap(),
            u64::try_from(printings).unwrap(),
            u64::try_from(rulings).unwrap()
        )
    );
    assert!(counts.rulings > 0);
    assert_eq!((counts.precon_decks, counts.precon_entries), (1, 5));
}
