//! Goal: single and batch card loading tolerate unknown and malformed ids, and a card's detail bundles exactly its own printings and rulings.

mod common;

use mtg_import::ids::OracleId;
use mtg_import::scryfall::RulingSource;
use sqlx::PgPool;

use common::{another_printing, db, names, oracle_of, record, ruling_stream, seeded};

#[sqlx::test]
async fn unknown_and_malformed_ids_are_simply_absent(pool: PgPool) {
    let database = seeded(pool).await;
    let cards = database.cards();
    let unknown = OracleId(uuid::Uuid::from_u128(1).to_string());
    let malformed = OracleId("not-a-uuid".into());

    for id in [&unknown, &malformed, &OracleId(String::new())] {
        assert_eq!(cards.get(id).await.unwrap(), None, "{id}");
        assert_eq!(cards.detail(id).await.unwrap(), None, "{id}");
    }
    assert!(cards.get_many(&[]).await.unwrap().is_empty());
    assert!(
        cards
            .get_many(&[unknown, malformed])
            .await
            .unwrap()
            .is_empty()
    );
}

#[sqlx::test]
async fn get_many_returns_each_known_card_once_in_name_order(pool: PgPool) {
    let database = seeded(pool).await;
    let ids = [
        oracle_of("Sol Ring"),
        OracleId("junk".into()),
        oracle_of("Shock"),
        oracle_of("Sol Ring"),
        oracle_of("Æther Vial"),
    ];

    let found = database.cards().get_many(&ids).await.unwrap();

    assert_eq!(names(&found), ["Æther Vial", "Shock", "Sol Ring"]);
}

#[sqlx::test]
async fn ids_are_accepted_in_any_uuid_spelling(pool: PgPool) {
    let database = seeded(pool).await;
    let canonical = oracle_of("Shock");
    let shouting = OracleId(canonical.as_str().to_uppercase());
    let simple = OracleId(canonical.as_str().replace('-', ""));

    for id in [canonical, shouting, simple] {
        let card = database.cards().get(&id).await.unwrap().unwrap();
        assert_eq!(card.name, "Shock");
        assert_eq!(
            card.oracle_id,
            oracle_of("Shock"),
            "ids come back in canonical form"
        );
    }
}

#[sqlx::test]
async fn detail_bundles_only_the_cards_own_printings_and_rulings(pool: PgPool) {
    let database = seeded(pool).await;
    let bolt = record("Lightning Bolt");
    database
        .importer()
        .import_cards(
            [
                another_printing(&bolt, 2, "lea", Some("1993-08-05")),
                another_printing(&record("Shock"), 3, "m20", Some("2019-07-12")),
            ]
            .into_iter()
            .map(Ok),
        )
        .await
        .unwrap();
    database
        .importer()
        .import_rulings(ruling_stream())
        .await
        .unwrap();

    let detail = database
        .cards()
        .detail(&bolt.oracle_id)
        .await
        .unwrap()
        .unwrap();

    assert_eq!(detail.card.name, "Lightning Bolt");
    let sets: Vec<_> = detail.printings.iter().map(|p| p.set.as_str()).collect();
    assert_eq!(sets, ["m10", "lea"]);
    assert!(
        detail
            .printings
            .iter()
            .all(|p| p.oracle_id == bolt.oracle_id)
    );
    assert_eq!(detail.rulings.len(), 1);
    assert_eq!(detail.rulings[0].source, RulingSource::Unknown);
    assert!(detail.rulings.iter().all(|r| r.oracle_id == bolt.oracle_id));

    let shock = database
        .cards()
        .detail(&oracle_of("Shock"))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(shock.printings.len(), 2);
    assert!(shock.rulings.is_empty());
}

#[sqlx::test]
async fn a_card_without_printings_or_rulings_still_loads(pool: PgPool) {
    let database = seeded(pool.clone()).await;
    sqlx::raw_sql("DELETE FROM printings")
        .execute(&pool)
        .await
        .unwrap();

    let detail = database
        .cards()
        .detail(&oracle_of("Mountain"))
        .await
        .unwrap()
        .unwrap();

    assert!(detail.printings.is_empty() && detail.rulings.is_empty());
    assert_eq!(detail.card.name, "Mountain");
}

#[sqlx::test]
async fn corrupt_stored_values_surface_as_errors_not_panics(pool: PgPool) {
    let database = db(pool.clone());
    database
        .importer()
        .import_cards(common::card_stream())
        .await
        .unwrap();
    sqlx::raw_sql("ALTER TABLE printings DROP CONSTRAINT printings_rarity_check")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::raw_sql("UPDATE printings SET rarity = 'ultra' WHERE oracle_id IN (SELECT oracle_id FROM cards WHERE name = 'Shock')")
        .execute(&pool)
        .await
        .unwrap();

    let error = database
        .cards()
        .detail(&oracle_of("Shock"))
        .await
        .unwrap_err();

    assert!(
        matches!(
            error,
            mtg_db::DbError::Corrupt {
                column: "printings.rarity",
                ..
            }
        ),
        "{error}"
    );
    let colors_error = {
        sqlx::raw_sql("ALTER TABLE cards DROP CONSTRAINT cards_colors_check")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::raw_sql("UPDATE cards SET colors = 99 WHERE name = 'Shock'")
            .execute(&pool)
            .await
            .unwrap();
        database.cards().get(&oracle_of("Shock")).await.unwrap_err()
    };
    assert!(matches!(
        colors_error,
        mtg_db::DbError::Corrupt {
            column: "cards.colors",
            ..
        }
    ));
}
