//! Goal: the art of a card's default printing and printings looked up by id or by set and number are exact: paper before digital, then the newest; unknown and malformed ids are absent; sets match in any case.

mod common;

use mtg_import::ids::{OracleId, PrintingId};
use mtg_import::scryfall::{CardRecord, ImageUris};
use sqlx::PgPool;

use common::{another_printing, oracle_of, record, repo, seeded};

fn ghost() -> OracleId {
    OracleId(uuid::Uuid::from_u128(77).to_string())
}

/// A printing of `base` whose art says which printing it is.
fn printing_with_art(
    base: &CardRecord,
    n: u128,
    set: &str,
    released: &str,
    digital: bool,
) -> CardRecord {
    let mut card = another_printing(base, n, set, Some(released));
    card.digital = digital;
    card.images = Some(ImageUris {
        normal: Some(format!("https://art.example/{set}.jpg")),
        ..ImageUris::default()
    });
    card
}

fn normal(images: &mtg_db::PrintingImages) -> Option<&str> {
    images.card.as_ref().and_then(|card| card.normal.as_deref())
}

#[sqlx::test]
async fn the_default_images_are_those_of_the_stored_printing(pool: PgPool) {
    seeded(pool.clone()).await;
    let ids = [
        oracle_of("Lightning Bolt"),
        oracle_of("Fire // Ice"),
        ghost(),
        OracleId("junk".into()),
    ];
    let images = repo(&pool).default_images(&ids).await.unwrap();

    assert_eq!(images.len(), 2, "unknown and malformed ids are absent");
    let bolt = normal(&images[&oracle_of("Lightning Bolt")]);
    assert!(bolt.is_some_and(|url| url.contains("e3285e6b")), "{bolt:?}");
    let split = &images[&oracle_of("Fire // Ice")];
    assert_eq!(
        split.faces.len(),
        2,
        "a card with two faces keeps one entry per face"
    );
    assert_eq!(repo(&pool).default_images(&[]).await.unwrap().len(), 0);
}

#[sqlx::test]
async fn paper_comes_before_digital_and_then_the_newest(pool: PgPool) {
    let database = seeded(pool.clone()).await;
    let bolt = record("Lightning Bolt");
    database
        .importer()
        .import_cards(
            [
                printing_with_art(&bolt, 2, "lea", "1993-08-05", false),
                printing_with_art(&bolt, 3, "j21", "2030-01-01", true),
                printing_with_art(&bolt, 4, "m25", "2025-01-01", false),
                printing_with_art(&bolt, 5, "xxx", "2024-01-01", false),
            ]
            .into_iter()
            .map(Ok),
        )
        .await
        .unwrap();

    let images = repo(&pool)
        .default_images(std::slice::from_ref(&bolt.oracle_id))
        .await
        .unwrap();
    assert_eq!(
        normal(&images[&bolt.oracle_id]),
        Some("https://art.example/m25.jpg"),
        "the newest paper printing beats older paper and newer digital ones"
    );
}

#[sqlx::test]
async fn printings_are_found_by_id_and_by_set_and_number(pool: PgPool) {
    seeded(pool.clone()).await;
    let bolt = record("Lightning Bolt");
    let repo = repo(&pool);

    let found = repo
        .printings(&[
            bolt.printing_id.clone(),
            PrintingId("junk".into()),
            PrintingId(uuid::Uuid::from_u128(9).to_string()),
        ])
        .await
        .unwrap();
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].oracle_id, bolt.oracle_id);
    assert_eq!(
        (found[0].set.as_str(), found[0].collector_number.as_str()),
        ("m10", "146")
    );

    for set in ["M10", "m10"] {
        let by_number = repo.printing_by_number(set, "146").await.unwrap();
        assert_eq!(
            by_number.map(|p| p.printing_id),
            Some(bolt.printing_id.clone()),
            "{set}"
        );
    }
    assert_eq!(repo.printing_by_number("m10", "999").await.unwrap(), None);
    assert_eq!(repo.printing_by_number("zzz", "146").await.unwrap(), None);
}
