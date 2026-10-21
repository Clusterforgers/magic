//! Goal: the name index built from the database resolves every recorded deck list exactly like the index built from the same Scryfall records in memory, so deck import can rely on stored cards.

use crate::common;

use mtg_import::decklist::parse;
use mtg_import::resolve::{Lookup, resolve_deck};
use mtg_import::scryfall::CardRecord;
use sqlx::PgPool;

use common::{another_card, card_index_pair, fixture_path, oracle_of, record, seeded};

const DECK_FILES: [&str; 10] = [
    "deck_plain.txt",
    "deck_plain_blank_sideboard.txt",
    "deck_arena.txt",
    "deck_moxfield.txt",
    "deck_archidekt.txt",
    "deck_mtgo.dek",
    "deck_arena_commander.txt",
    "deck_archidekt_commander.txt",
    "deck_unicode_and_typos.txt",
    "deck_messy.txt",
];

#[sqlx::test]
async fn every_recorded_deck_resolves_identically_through_both_indexes(pool: PgPool) {
    let database = seeded(pool).await;
    let (stored, in_memory) = card_index_pair(&database).await;
    for file in DECK_FILES {
        let text = std::fs::read_to_string(fixture_path(file)).unwrap();
        let deck = parse(&text);
        assert!(!deck.entries.is_empty(), "{file} should have entries");
        assert_eq!(
            resolve_deck(&stored, &deck),
            resolve_deck(&in_memory, &deck),
            "{file}"
        );
    }
}

#[sqlx::test]
async fn the_precon_deck_resolves_identically_through_both_indexes(pool: PgPool) {
    let database = seeded(pool).await;
    let (stored, in_memory) = card_index_pair(&database).await;
    let deck = common::precon_deck().to_parsed_deck();

    let resolution = resolve_deck(&stored, &deck);

    assert_eq!(resolution, resolve_deck(&in_memory, &deck));
    assert!(
        resolution.unresolved.is_empty(),
        "{:?}",
        resolution.unresolved
    );
}

#[sqlx::test]
async fn the_index_holds_deck_cards_only(pool: PgPool) {
    let database = seeded(pool).await;

    let index = database.cards().card_index().await.unwrap();

    assert_eq!(index.len(), 20);
    for excluded in ["Goblin", "Elspeth, Sun's Champion Emblem"] {
        assert_eq!(index.find(excluded), Lookup::Missing, "{excluded}");
    }
    assert!(
        matches!(index.find("Lightning Bolt"), Lookup::Found(card) if card.oracle_id == oracle_of("Lightning Bolt"))
    );
}

#[sqlx::test]
async fn the_index_knows_the_stored_printings_of_deck_cards(pool: PgPool) {
    let database = seeded(pool).await;
    let (stored, in_memory) = card_index_pair(&database).await;
    let records = common::fixture_records();
    let mut checked = 0;
    let numbered = |r: &&CardRecord| !r.set.is_empty() && !r.collector_number.is_empty();
    for record in records
        .iter()
        .filter(numbered)
        .filter(|r| r.layout.is_deck_card())
    {
        let found = stored.find_printing(&record.set.to_uppercase(), &record.collector_number);
        assert!(
            matches!(&found, Lookup::Found(card) if card.oracle_id == record.oracle_id),
            "{} ({} {}): {found:?}",
            record.name,
            record.set,
            record.collector_number
        );
        assert_eq!(
            found,
            in_memory.find_printing(&record.set, &record.collector_number)
        );
        checked += 1;
    }
    assert!(checked >= 15, "only {checked} printings were checked");
    for token in records.iter().filter(|r| !r.layout.is_deck_card()) {
        assert_eq!(
            stored.find_printing(&token.set, &token.collector_number),
            Lookup::Missing,
            "{}",
            token.name
        );
    }
}

#[sqlx::test]
async fn an_empty_database_gives_an_empty_index(pool: PgPool) {
    let index = common::db(pool).cards().card_index().await.unwrap();

    assert!(index.is_empty());
    assert_eq!(index.find("Lightning Bolt"), Lookup::Missing);
    assert_eq!(index.suggest("lightning bolt", 5).len(), 0);
}

#[sqlx::test]
async fn the_index_follows_later_imports(pool: PgPool) {
    let database = seeded(pool).await;
    let before = database.cards().card_index().await.unwrap();
    assert_eq!(before.find("Brand New Card"), Lookup::Missing);

    database
        .importer()
        .import_cards([Ok(another_card(&record("Shock"), 77, "Brand New Card"))])
        .await
        .unwrap();

    let after = database.cards().card_index().await.unwrap();
    assert_eq!(after.len(), before.len() + 1);
    assert!(matches!(after.find("brand new card"), Lookup::Found(_)));
}

#[sqlx::test]
async fn stored_cards_feed_fuzzy_suggestions_for_unresolved_names(pool: PgPool) {
    let database = seeded(pool).await;
    let (stored, in_memory) = card_index_pair(&database).await;
    let cases = [
        ("Lightnin Bolt", "Lightning Bolt"),
        ("Shok", "Shock"),
        ("Aether Vial", "Æther Vial"),
        ("Jace the Mind Sculptor", "Jace, the Mind Sculptor"),
    ];
    for (typed, expected) in cases {
        let suggestions = stored.suggest(typed, 3);
        assert_eq!(suggestions[0].name, expected, "{typed}");
        assert_eq!(suggestions, in_memory.suggest(typed, 3), "{typed}");
    }
}

#[sqlx::test]
async fn the_version_changes_exactly_when_cards_or_printings_do(pool: PgPool) {
    let database = seeded(pool.clone()).await;
    let cards = database.cards();
    let seeded_version = cards.index_version().await.unwrap();

    database
        .importer()
        .import_cards(common::card_stream())
        .await
        .unwrap();
    let rewritten = cards.index_version().await.unwrap();
    assert_eq!(
        rewritten, seeded_version,
        "unchanged rows are not rewritten"
    );

    let printing = common::another_printing(&record("Shock"), 50, "sld", None);
    database
        .importer()
        .import_cards([Ok(printing)])
        .await
        .unwrap();
    let printed = cards.index_version().await.unwrap();
    assert_ne!(printed, seeded_version, "a printing was added");

    let card = another_card(&record("Shock"), 77, "Brand New Card");
    database.importer().import_cards([Ok(card)]).await.unwrap();
    let carded = cards.index_version().await.unwrap();
    assert_ne!(carded, printed, "a card was added");

    sqlx::query("DELETE FROM printings WHERE collector_number = '50'")
        .execute(&pool)
        .await
        .unwrap();
    assert_ne!(
        cards.index_version().await.unwrap(),
        carded,
        "a printing was removed"
    );
}
