//! Goal: the printing that stands for a card is the newest paper one that has a picture (else a digital one), a card with no picture anywhere has none, and tokens are found by name among the cards that are not deck cards, each with the printing that shows it.

use crate::common;

use mtg_import::ids::OracleId;
use mtg_import::scryfall::ImageUris;
use sqlx::PgPool;

use common::{another_printing, oracle_of, record, repo, seeded};

fn art(tag: &str) -> ImageUris {
    ImageUris {
        normal: Some(format!(
            "https://cards.scryfall.io/normal/front/a/b/{tag}.jpg?1"
        )),
        ..ImageUris::default()
    }
}

fn normal(images: &mtg_db::PrintingImages) -> Option<&str> {
    images.card.as_ref().and_then(|card| card.normal.as_deref())
}

#[sqlx::test]
async fn the_newest_paper_printing_with_a_picture_stands_for_a_card(pool: PgPool) {
    let database = seeded(pool.clone()).await;
    let bolt = record("Lightning Bolt");
    let mut printings = vec![
        another_printing(&bolt, 2, "lea", Some("1993-08-05")),
        another_printing(&bolt, 3, "j21", Some("2030-01-01")),
        another_printing(&bolt, 4, "m25", Some("2025-01-01")),
        another_printing(&bolt, 5, "blk", Some("2029-01-01")),
    ];
    printings[0].images = Some(art("lea"));
    printings[1].images = Some(art("j21"));
    printings[1].digital = true;
    printings[2].images = Some(art("m25"));
    printings[3].images = None;
    let chosen = printings[2].printing_id.clone();
    database
        .importer()
        .import_cards(printings.into_iter().map(Ok))
        .await
        .unwrap();

    let found = repo(&pool)
        .picture_printing(&bolt.oracle_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        normal(&found.images),
        Some("https://cards.scryfall.io/normal/front/a/b/m25.jpg?1"),
        "paper before digital, a picture before none, then the newest"
    );
    assert_eq!(found.printing_id, chosen);
}

#[sqlx::test]
async fn a_digital_printing_stands_in_when_no_paper_one_has_a_picture(pool: PgPool) {
    let database = seeded(pool.clone()).await;
    let shock = record("Shock");
    let mut paper = another_printing(&shock, 2, "m19", Some("2018-07-13"));
    paper.images = None;
    let mut digital = another_printing(&shock, 3, "j21", Some("2021-01-01"));
    digital.digital = true;
    digital.images = Some(art("j21"));
    let mut original = shock.clone();
    original.images = None;
    database
        .importer()
        .import_cards([original, paper, digital].into_iter().map(Ok))
        .await
        .unwrap();
    let found = repo(&pool)
        .picture_printing(&shock.oracle_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        normal(&found.images),
        Some("https://cards.scryfall.io/normal/front/a/b/j21.jpg?1")
    );
}

#[sqlx::test]
async fn unknown_malformed_and_pictureless_cards_have_no_printing(pool: PgPool) {
    let database = seeded(pool.clone()).await;
    let mut bare = record("Island");
    bare.images = None;
    bare.front.images = None;
    database.importer().import_cards([Ok(bare)]).await.unwrap();
    let repo = repo(&pool);
    let ghost = OracleId(uuid::Uuid::from_u128(77).to_string());
    for id in [ghost, OracleId("junk".into()), oracle_of("Island")] {
        assert!(
            repo.picture_printing(&id).await.unwrap().is_none(),
            "{id:?}"
        );
    }
}

#[sqlx::test]
async fn tokens_are_found_by_a_normalized_name_with_their_printing(pool: PgPool) {
    seeded(pool.clone()).await;
    let repo = repo(&pool);
    for name in ["Goblin", "GOBLIN", "  goblin "] {
        let found = repo.token_pictures(name).await.unwrap();
        assert_eq!(found.len(), 1, "{name:?}");
        assert_eq!(found[0].card.name, "Goblin");
        assert_eq!(found[0].card.type_line, "Token Creature \u{2014} Goblin");
        assert_eq!(found[0].card.faces[0].power.as_deref(), Some("1"));
        assert_eq!(
            normal(&found[0].pictures.images),
            Some(
                "https://cards.scryfall.io/normal/front/a/6/a68ee7f0-3d3f-4084-a740-35d1cc9d48d6.jpg?1"
            )
        );
    }
}

#[sqlx::test]
async fn deck_cards_unknown_and_blank_names_are_no_tokens(pool: PgPool) {
    seeded(pool.clone()).await;
    let repo = repo(&pool);
    for name in ["Lightning Bolt", "No Such Token", "", "   "] {
        assert!(
            repo.token_pictures(name).await.unwrap().is_empty(),
            "{name:?}"
        );
    }
}
