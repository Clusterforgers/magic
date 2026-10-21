//! Goal: `PreconRepo::all` returns what `list` and `get` return deck by deck, entries that match no card included, and `PreconRepo::version` changes with every edit of a deck or an entry and with nothing else.

use crate::common;

use mtg_db::PreconDeckId;
use mtg_import::mtgjson::PreconDeck;
use sqlx::PgPool;

use common::precon::{deck, entry, ingest};
use common::{precon_deck, seeded};

fn small(name: &str, code: &str, cards: &[(&str, u32)]) -> PreconDeck {
    let main = cards.iter().map(|(card, n)| entry(card, *n)).collect();
    deck(name, code, Some("2024-01-01"), main)
}

#[sqlx::test]
async fn all_equals_list_and_get_deck_by_deck(pool: PgPool) {
    let database = seeded(pool).await;
    ingest(
        &database,
        vec![
            precon_deck(),
            small("Burn", "TST", &[("Lightning Bolt", 4), ("Shock", 2)]),
            small("Ghost", "TST", &[("No Such Card", 3), ("Shock", 1)]),
        ],
    )
    .await;
    let repo = database.precons();

    let all = repo.all().await.unwrap();

    let listed = repo.list().await.unwrap();
    assert_eq!(all.len(), 3);
    assert_eq!(
        all.iter().map(|d| d.summary.clone()).collect::<Vec<_>>(),
        listed
    );
    for deck in &all {
        let one = repo.get(deck.summary.id).await.unwrap().unwrap();
        assert_eq!(deck, &one, "{}", deck.summary.name);
    }
    let ghost = all.iter().find(|d| d.summary.name == "Ghost").unwrap();
    assert_eq!(ghost.summary.unresolved_count, 3);
    assert!(ghost.entries.iter().any(|e| e.card.is_none()));
}

#[sqlx::test]
async fn all_of_no_precons_is_empty(pool: PgPool) {
    let database = seeded(pool).await;
    assert_eq!(database.precons().all().await.unwrap().len(), 0);
}

#[sqlx::test]
async fn the_version_changes_with_every_edit_and_with_nothing_else(pool: PgPool) {
    let database = seeded(pool).await;
    let repo = database.precons();
    let empty = repo.version().await.unwrap();
    assert_eq!(repo.version().await.unwrap(), empty);

    ingest(&database, vec![small("Burn", "TST", &[("Shock", 2)])]).await;
    let one = repo.version().await.unwrap();
    assert_ne!(one, empty);
    assert_eq!(repo.version().await.unwrap(), one);

    // The same file again changes nothing.
    ingest(&database, vec![small("Burn", "TST", &[("Shock", 2)])]).await;
    assert_eq!(repo.version().await.unwrap(), one);

    // A changed count, a changed card, a changed header and a second deck each count.
    let mut seen = vec![empty, one];
    for edited in [
        vec![small("Burn", "TST", &[("Shock", 3)])],
        vec![small("Burn", "TST", &[("Lightning Bolt", 3)])],
        vec![deck(
            "Burn",
            "TST",
            Some("2025-02-02"),
            vec![entry("Lightning Bolt", 3)],
        )],
        vec![small("Other", "TST", &[("Shock", 1)])],
    ] {
        ingest(&database, edited).await;
        let now = repo.version().await.unwrap();
        assert!(!seen.contains(&now), "a new state has a new version");
        seen.push(now);
    }
}

#[sqlx::test]
async fn removing_a_deck_changes_the_version(pool: PgPool) {
    let database = seeded(pool.clone()).await;
    let report = ingest(&database, vec![small("Burn", "TST", &[("Shock", 2)])]).await;
    let id: PreconDeckId = report.decks[0].deck_id;
    let before = database.precons().version().await.unwrap();

    sqlx::query("DELETE FROM precon_decks WHERE id = $1")
        .bind(id.0)
        .execute(&pool)
        .await
        .unwrap();

    assert_ne!(database.precons().version().await.unwrap(), before);
    assert_eq!(database.precons().all().await.unwrap().len(), 0);
}
