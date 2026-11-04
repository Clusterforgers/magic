//! Goal: `coverage-decks` counts a card once per saved deck and once per owner, leaves fully automatic cards out of the list, counts them in the total line, ranks the most used first, writes the same as JSON, prints no account and says so when no deck is saved.

#![allow(clippy::unwrap_used)]

use crate::common;

use mtg_cli::args::CoverageDecksArgs;
use mtg_cli::cache::Cache;
use mtg_cli::coverage_decks::coverage_decks;
use mtg_cli::ingest::{IngestOptions, ingest};
use mtg_db::Database;
use sqlx::PgPool;
use sqlx::types::uuid::Uuid;

use common::{fixture_cache, progress};

async fn seeded(pool: PgPool, dir: &std::path::Path) -> Database {
    let cache: Cache = fixture_cache(dir);
    let db = Database::from_pool(pool);
    let (progress, _) = progress();
    let options = IngestOptions {
        force: false,
        batch_size: 500,
    };
    ingest(&cache, &db, &options, &progress)
        .await
        .unwrap_or_else(|e| panic!("ingest runs: {e}"));
    db
}

/// A deck of `owner` holding one line for each named card.
async fn deck(db: &Database, owner: u128, cards: &[&str]) {
    let id: Uuid = sqlx::query_scalar(
        "INSERT INTO decks (owner, name, format) VALUES ($1, 'Deck', 'freeform') RETURNING id",
    )
    .bind(Uuid::from_u128(owner))
    .fetch_one(db.pool())
    .await
    .unwrap();
    for (position, name) in cards.iter().enumerate() {
        sqlx::query(
            "INSERT INTO deck_cards (deck_id, position, zone, oracle_id, count)
             SELECT $1, $2, 'main', oracle_id, 1 FROM cards WHERE name = $3",
        )
        .bind(id)
        .bind(i32::try_from(position).unwrap())
        .bind(*name)
        .execute(db.pool())
        .await
        .unwrap();
    }
}

fn args(json: bool) -> CoverageDecksArgs {
    CoverageDecksArgs { json }
}

#[sqlx::test]
async fn a_card_counts_per_deck_and_owner_and_automatic_cards_are_left_out(pool: PgPool) {
    let dir = tempfile::tempdir().unwrap();
    let db = seeded(pool, dir.path()).await;
    deck(
        &db,
        1,
        &["Lightning Bolt", "Sol Ring", "Jace, the Mind Sculptor"],
    )
    .await;
    deck(&db, 2, &["Sol Ring", "Jace, the Mind Sculptor"]).await;
    deck(&db, 1, &["Jace, the Mind Sculptor"]).await;

    let text = coverage_decks(&db, &args(false)).await.unwrap();

    let jace = text
        .lines()
        .find(|line| line.contains("Jace, the Mind Sculptor"))
        .unwrap_or_else(|| panic!("{text}"));
    let fields: Vec<&str> = jace.split_whitespace().collect();
    assert_eq!(&fields[2..4], ["3", "2"], "three decks, two owners: {jace}");
    let sol = text.lines().find(|line| line.contains("Sol Ring")).unwrap();
    assert_eq!(
        &sol.split_whitespace().collect::<Vec<_>>()[2..4],
        ["2", "2"]
    );
    assert!(
        text.find("Jace").unwrap() < text.find("Sol Ring").unwrap(),
        "the most used first: {text}"
    );
    assert!(!text.contains("Lightning Bolt"), "automatic: {text}");
    assert!(
        text.ends_with(
            "3 distinct cards in saved decks: 1 fully automatic, 0 partly by hand, 2 by hand"
        ),
        "{text}"
    );
}

#[sqlx::test]
async fn json_has_the_same_cards_and_no_account(pool: PgPool) {
    let dir = tempfile::tempdir().unwrap();
    let db = seeded(pool, dir.path()).await;
    deck(&db, 7, &["Lightning Bolt", "Sol Ring"]).await;

    let text = coverage_decks(&db, &args(true)).await.unwrap();

    let value: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(value["cards"][0]["name"], "Sol Ring");
    assert_eq!(value["cards"][0]["decks"], 1);
    assert_eq!(value["totals"]["cards"], 2);
    assert!(!text.contains(&Uuid::from_u128(7).to_string()), "{text}");
}

#[sqlx::test]
async fn with_no_saved_deck_the_total_says_zero(pool: PgPool) {
    let dir = tempfile::tempdir().unwrap();
    let db = seeded(pool, dir.path()).await;

    let text = coverage_decks(&db, &args(false)).await.unwrap();

    assert!(
        text.ends_with(
            "0 distinct cards in saved decks: 0 fully automatic, 0 partly by hand, 0 by hand"
        ),
        "{text}"
    );
}
