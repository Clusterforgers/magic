//! Goal: paging through search results is stable: every page window is exact, ties never shuffle between pages, and a window past the end is empty but still reports the total.

use crate::common;

use std::collections::BTreeSet;

use mtg_db::cards::MAX_PAGE_SIZE;
use mtg_db::{CardSearch, Page, SortKey};
use sqlx::PgPool;

use common::{another_card, db, names, record, repo, seeded};

/// `(limit, offset, expected number of cards)` over the 20 deck cards.
const WINDOWS: [(u32, u32, usize); 9] = [
    (1, 0, 1),
    (5, 0, 5),
    (5, 15, 5),
    (5, 18, 2),
    (5, 20, 0),
    (5, 500, 0),
    (20, 0, 20),
    (100, 0, 20),
    (3, u32::MAX, 0),
];

#[sqlx::test]
async fn windows_have_the_exact_size_and_always_report_the_total(pool: PgPool) {
    seeded(pool.clone()).await;
    let repo = repo(&pool);
    for (limit, offset, expected) in WINDOWS {
        let page = repo
            .search(&CardSearch::default(), Page::new(limit, offset))
            .await
            .unwrap();
        assert_eq!(page.cards.len(), expected, "limit {limit} offset {offset}");
        assert_eq!(page.total, 20, "limit {limit} offset {offset}");
        assert_eq!((page.page.limit(), page.page.offset()), (limit, offset));
    }
}

#[sqlx::test]
async fn consecutive_pages_tile_the_result_without_gaps_or_repeats(pool: PgPool) {
    seeded(pool.clone()).await;
    let repo = repo(&pool);
    let everything = names(
        &repo
            .search(&CardSearch::default(), Page::new(MAX_PAGE_SIZE, 0))
            .await
            .unwrap()
            .cards,
    );

    for limit in 1..=21 {
        let mut stitched = Vec::new();
        let mut page = Page::new(limit, 0);
        loop {
            let result = repo.search(&CardSearch::default(), page).await.unwrap();
            if result.cards.is_empty() {
                break;
            }
            stitched.extend(names(&result.cards));
            page = page.next();
        }
        assert_eq!(stitched, everything, "limit {limit}");
    }
}

#[sqlx::test]
async fn ties_in_the_sort_key_never_shuffle_between_pages(pool: PgPool) {
    let database = db(pool.clone());
    let base = record("Shock");
    // Sixty cards that all share one mana value and differ only by name.
    let cards = (0..60).map(|n| {
        Ok(another_card(
            &base,
            1_000 + n,
            &format!("Same Cost {:02}", (n * 37) % 60),
        ))
    });
    database.importer().import_cards(cards).await.unwrap();
    let repo = database.cards();
    let search = CardSearch {
        sort: SortKey::ManaValue,
        ..CardSearch::default()
    };

    let mut seen = Vec::new();
    for round in 0..3 {
        let mut stitched = Vec::new();
        let mut page = Page::new(7, 0);
        loop {
            let result = repo.search(&search, page).await.unwrap();
            if result.cards.is_empty() {
                break;
            }
            stitched.extend(names(&result.cards));
            page = page.next();
        }
        assert_eq!(stitched.len(), 60, "round {round}");
        let distinct: BTreeSet<_> = stitched.iter().collect();
        assert_eq!(distinct.len(), 60, "round {round} repeated a card");
        seen.push(stitched);
    }
    assert!(
        seen.windows(2).all(|pair| pair[0] == pair[1]),
        "order differs between runs"
    );
    let mut sorted = seen[0].clone();
    sorted.sort();
    assert_eq!(seen[0], sorted, "ties are ordered by name");
}

#[sqlx::test]
async fn oversized_and_zero_limits_are_clamped(pool: PgPool) {
    let database = db(pool);
    let base = record("Shock");
    let cards = (0..250).map(|n| Ok(another_card(&base, 5_000 + n, &format!("Bulk {n:03}"))));
    database.importer().import_cards(cards).await.unwrap();
    let repo = database.cards();

    let huge = repo
        .search(&CardSearch::default(), Page::new(u32::MAX, 0))
        .await
        .unwrap();
    let zero = repo
        .search(&CardSearch::default(), Page::new(0, 0))
        .await
        .unwrap();

    assert_eq!(huge.cards.len(), MAX_PAGE_SIZE as usize);
    assert_eq!(huge.total, 250);
    assert_eq!(zero.cards.len(), 1);
    assert_eq!(Page::default().limit(), mtg_db::cards::DEFAULT_PAGE_SIZE);
}
