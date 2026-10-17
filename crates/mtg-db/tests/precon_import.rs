//! Goal: precon decks ingest idempotently and atomically, load in deck order, list newest first, and skip unusable deck files.

mod common;

use mtg_db::{DeckBoard, PreconDeckId, RunStatus};
use mtg_import::ids::PrintingId;
use mtg_import::mtgjson::{MtgjsonError, PreconDeck, parse_deck_str};
use sqlx::PgPool;

use common::precon::{deck, entry, ingest};
use common::{db, oracle_of, precon_deck, seeded, snapshot};

#[sqlx::test]
async fn the_fixture_deck_resolves_completely_and_loads_in_deck_order(pool: PgPool) {
    let database = seeded(pool).await;

    let report = ingest(&database, vec![precon_deck()]).await;

    let [ingested] = report.decks.as_slice() else {
        panic!("one deck expected");
    };
    assert_eq!(
        (ingested.code.as_str(), ingested.name.as_str()),
        ("C16", "Breed Lethality")
    );
    assert_eq!(ingested.entries, 5);
    assert_eq!(ingested.unresolved.len(), 0);

    let loaded = database
        .precons()
        .get(ingested.deck_id)
        .await
        .unwrap()
        .unwrap();
    let summary = &loaded.summary;
    assert_eq!(summary.deck_type.as_deref(), Some("Commander Deck"));
    assert_eq!(
        summary.release_date.map(|d| d.to_string()).as_deref(),
        Some("2016-11-11")
    );
    assert_eq!((summary.card_count, summary.unresolved_count), (40, 0));

    let lines: Vec<_> = loaded
        .entries
        .iter()
        .map(|e| {
            (
                e.position,
                e.board,
                e.count,
                e.card.as_ref().unwrap().name.as_str(),
            )
        })
        .collect();
    assert_eq!(
        lines,
        [
            (0, DeckBoard::Commander, 1, "Atraxa, Praetors' Voice"),
            (1, DeckBoard::Main, 1, "Sol Ring"),
            (2, DeckBoard::Main, 1, "Arcane Signet"),
            (3, DeckBoard::Main, 36, "Forest"),
            (4, DeckBoard::Main, 1, "Fire // Ice"),
        ]
    );
    let sol_ring = &loaded.entries[1];
    assert!(sol_ring.is_foil);
    assert_eq!(sol_ring.set_code.as_deref(), Some("C16"));
    assert_eq!(sol_ring.collector_number.as_deref(), Some("266"));
    assert_eq!(
        sol_ring.printing_id.as_ref().map(PrintingId::as_str),
        Some("7a0bc8ee-d092-431e-88fa-f60f951a23e5")
    );
    assert_eq!(
        loaded.entries[4].card.as_ref().unwrap().oracle_id,
        oracle_of("Fire // Ice")
    );
    assert_eq!(
        loaded.entries[4].printing_id, None,
        "the file names no printing"
    );

    let run = database
        .import_runs()
        .get(report.run_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(run.status, RunStatus::Succeeded);
    assert_eq!(run.counts["decks"][0]["name"], "Breed Lethality");
}

/// `(entry, expected outcome)`: `None` means resolved to the named card, `Some` the reason it is not.
#[sqlx::test]
async fn reingesting_a_deck_is_idempotent_and_replaces_its_cards(pool: PgPool) {
    let database = seeded(pool.clone()).await;
    let first = ingest(&database, vec![precon_deck()]).await;
    let decks = snapshot(&pool, "precon_decks", "id", true).await;
    let cards = snapshot(&pool, "precon_deck_cards", "deck_id, position", true).await;

    let again = ingest(&database, vec![precon_deck()]).await;

    assert_eq!(again.decks[0].deck_id, first.decks[0].deck_id);
    assert_eq!(snapshot(&pool, "precon_decks", "id", true).await, decks);
    assert_eq!(
        snapshot(&pool, "precon_deck_cards", "deck_id, position", true).await,
        cards
    );
    assert_eq!(database.precons().list().await.unwrap().len(), 1);

    let mut revised = precon_deck();
    revised.main_board.retain(|card| card.name != "Forest");
    revised.main_board.push(entry("Shock", 2));
    revised.deck_type = Some("Revised".into());
    let third = ingest(&database, vec![revised]).await;

    assert_eq!(third.decks[0].deck_id, first.decks[0].deck_id);
    let loaded = database
        .precons()
        .get(first.decks[0].deck_id)
        .await
        .unwrap()
        .unwrap();
    let names: Vec<_> = loaded
        .entries
        .iter()
        .map(|e| e.card_name.as_str())
        .collect();
    assert_eq!(
        names,
        [
            "Atraxa, Praetors' Voice",
            "Sol Ring",
            "Arcane Signet",
            "Fire // Ice",
            "Shock"
        ]
    );
    assert_eq!(loaded.summary.deck_type.as_deref(), Some("Revised"));
    assert_eq!(loaded.summary.card_count, 6);
}

#[sqlx::test]
async fn decks_list_newest_first_then_by_name_with_undated_last(pool: PgPool) {
    let database = seeded(pool).await;
    let one = |name: &str, code: &str, date: Option<&str>| {
        deck(name, code, date, vec![entry("Shock", 1)])
    };

    ingest(
        &database,
        vec![
            one("Zeta", "A", Some("2020-01-01")),
            one("Undated", "B", None),
            one("Alpha", "C", Some("2020-01-01")),
            one("Newest", "D", Some("2024-06-01")),
            one("Oldest", "E", Some("1999-03-03")),
        ],
    )
    .await;

    let names: Vec<_> = database
        .precons()
        .list()
        .await
        .unwrap()
        .into_iter()
        .map(|d| d.name)
        .collect();
    assert_eq!(names, ["Newest", "Alpha", "Zeta", "Oldest", "Undated"]);
}

#[sqlx::test]
async fn the_same_name_in_different_products_is_a_different_deck(pool: PgPool) {
    let database = seeded(pool).await;
    let report = ingest(
        &database,
        vec![
            deck("Twins", "AAA", None, vec![entry("Shock", 1)]),
            deck("Twins", "BBB", None, vec![entry("Shock", 2)]),
        ],
    )
    .await;

    assert_ne!(report.decks[0].deck_id, report.decks[1].deck_id);
    assert_eq!(database.precons().list().await.unwrap().len(), 2);
}

#[sqlx::test]
async fn unusable_deck_files_are_skipped_and_good_ones_still_load(pool: PgPool) {
    let database = seeded(pool).await;
    let parse_error: MtgjsonError = parse_deck_str("{}").unwrap_err();
    let items: Vec<Result<PreconDeck, MtgjsonError>> = vec![
        Ok(deck(
            "Good",
            "G",
            Some("2020-01-01"),
            vec![entry("Shock", 1)],
        )),
        Err(parse_error),
        Ok(deck(
            "Bad date",
            "D",
            Some("2020-02-31"),
            vec![entry("Shock", 1)],
        )),
        Ok(deck(
            "Huge count",
            "H",
            None,
            vec![entry("Shock", u32::MAX)],
        )),
        Ok(deck("Also good", "A", None, vec![entry("Sol Ring", 1)])),
    ];

    let report = database.importer().import_precons(items).await.unwrap();

    assert_eq!(report.decks.len(), 2);
    assert_eq!(report.skipped.count, 3);
    let positions: Vec<u64> = report.skipped.samples.iter().map(|s| s.position).collect();
    assert_eq!(positions, [1, 2, 3]);
    assert!(report.skipped.samples[1].reason.contains("Bad date"));
    let listed: Vec<_> = database
        .precons()
        .list()
        .await
        .unwrap()
        .into_iter()
        .map(|d| d.name)
        .collect();
    assert_eq!(listed, ["Good", "Also good"]);
}

#[sqlx::test]
async fn a_failing_write_rolls_the_whole_deck_back(pool: PgPool) {
    let database = seeded(pool.clone()).await;
    ingest(
        &database,
        vec![deck("Stable", "S", None, vec![entry("Shock", 1)])],
    )
    .await;
    sqlx::raw_sql(
        "ALTER TABLE precon_deck_cards ADD CONSTRAINT poisoned CHECK (card_name <> 'Sol Ring')",
    )
    .execute(&pool)
    .await
    .unwrap();

    let error = database
        .importer()
        .import_precons([Ok(deck("Stable", "S", None, vec![entry("Sol Ring", 1)]))])
        .await
        .unwrap_err();

    assert!(matches!(
        error,
        mtg_db::ImportError::Database {
            run_id: Some(_),
            ..
        }
    ));
    let id = database.precons().list().await.unwrap()[0].id;
    let loaded = database.precons().get(id).await.unwrap().unwrap();
    let names: Vec<_> = loaded
        .entries
        .iter()
        .map(|e| e.card_name.as_str())
        .collect();
    assert_eq!(names, ["Shock"], "the old cards survive a failed re-ingest");
}

#[sqlx::test]
async fn absent_decks_are_none_and_an_empty_database_lists_nothing(pool: PgPool) {
    let precons = db(pool).precons();
    assert_eq!(precons.list().await.unwrap().len(), 0);
    assert_eq!(precons.get(PreconDeckId(1)).await.unwrap(), None);
}
