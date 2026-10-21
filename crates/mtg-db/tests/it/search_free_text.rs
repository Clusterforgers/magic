//! Goal: free text finds cards by name or by rules text, relevance puts the card named by the text first, then names that start with it, then names that contain it, then rules-text matches, and paging through a relevance order never repeats or skips a card.

use crate::common;

use mtg_db::cards::MAX_PAGE_SIZE;
use mtg_db::{CardSearch, Page, SortKey};
use mtg_import::scryfall::CardRecord;
use sqlx::PgPool;

use common::search::{Case, found, ordered, run, with};
use common::{another_card, db, names, record, repo, seeded};

fn free(text: &str, sort: SortKey) -> CardSearch {
    with(|s| {
        s.text = Some(text.to_owned());
        s.sort = sort;
    })
}

/// Four cards for the text `bolt`, named so that alphabetical order and relevance disagree.
fn bolt_cards() -> Vec<CardRecord> {
    let base = record("Shock");
    let make = |n: u128, name: &str, text: &str| {
        let mut card = another_card(&base, n, name);
        text.clone_into(&mut card.front.oracle_text);
        card
    };
    vec![
        make(1, "Aardvark Bolt", "Draw a card."),
        make(
            2,
            "Ambush",
            "Deals 3 damage to target creature, like a bolt.",
        ),
        make(3, "Bolt", "Draw a card."),
        make(4, "Boltwing", "Flying"),
    ]
}

#[sqlx::test]
async fn free_text_matches_names_and_rules_text(pool: PgPool) {
    seeded(pool.clone()).await;
    let cases = vec![
        found("a name", free("sol ring", SortKey::Name), &["Sol Ring"]),
        found(
            "a name fragment in any case, accents folded",
            free("AETHER", SortKey::Name),
            &["Æther Vial"],
        ),
        found(
            "a word of the rules text",
            free("damage", SortKey::Name),
            &[
                "Bonecrusher Giant // Stomp",
                "Fire // Ice",
                "Lightning Bolt",
                "Shock",
            ],
        ),
        found(
            "a name or a word of a text",
            free("bolt", SortKey::Name),
            &["Lightning Bolt"],
        ),
        found("nothing", free("zzzzzz", SortKey::Name), &[]),
        found(
            "blank text is no filter",
            free("   ", SortKey::Name),
            &common::search::ALL_DECK_CARDS,
        ),
        found(
            "injection attempts are just words",
            free("'; DROP TABLE cards; --", SortKey::Name),
            &[],
        ),
        found(
            "other filters still apply",
            with(|s| {
                s.text = Some("damage".into());
                s.max_mana_value = Some(1.0);
            }),
            &["Lightning Bolt", "Shock"],
        ),
    ];
    run(&repo(&pool), cases).await;
}

#[sqlx::test]
async fn relevance_ranks_the_name_before_the_rules_text(pool: PgPool) {
    db(pool.clone())
        .importer()
        .import_cards(bolt_cards().into_iter().map(Ok))
        .await
        .unwrap();
    let cases: Vec<Case> = vec![
        ordered(
            "relevance: exact name, prefix, contained, then text only",
            free("bolt", SortKey::Relevance),
            &["Bolt", "Boltwing", "Aardvark Bolt", "Ambush"],
        ),
        ordered(
            "name order is alphabetical",
            free("bolt", SortKey::Name),
            &["Aardvark Bolt", "Ambush", "Bolt", "Boltwing"],
        ),
        ordered(
            "relevance ignores case and spacing in the text",
            free("  BOLT ", SortKey::Relevance),
            &["Bolt", "Boltwing", "Aardvark Bolt", "Ambush"],
        ),
        ordered(
            "without text relevance is name order",
            with(|s| s.sort = SortKey::Relevance),
            &["Aardvark Bolt", "Ambush", "Bolt", "Boltwing"],
        ),
        ordered(
            "a text that only the rules hold",
            free("damage", SortKey::Relevance),
            &["Ambush"],
        ),
    ];
    run(&repo(&pool), cases).await;
}

#[sqlx::test]
async fn pages_of_a_relevance_order_tile_the_whole_result(pool: PgPool) {
    let base = record("Shock");
    let mut cards = bolt_cards();
    for n in 10..40 {
        let mut card = another_card(&base, n, &format!("Zap {n} of the bolt"));
        "Draw a card.".clone_into(&mut card.front.oracle_text);
        cards.push(card);
    }
    db(pool.clone())
        .importer()
        .import_cards(cards.into_iter().map(Ok))
        .await
        .unwrap();
    let repo = repo(&pool);
    let search = free("bolt", SortKey::Relevance);
    let whole = names(
        &repo
            .search(&search, Page::new(MAX_PAGE_SIZE, 0))
            .await
            .unwrap()
            .cards,
    );
    assert_eq!(whole.len(), 34);
    assert_eq!(whole[..2], ["Bolt", "Boltwing"]);

    let mut paged = Vec::new();
    let mut page = Page::new(7, 0);
    loop {
        let result = repo.search(&search, page).await.unwrap();
        assert_eq!(result.total, 34);
        if result.cards.is_empty() {
            break;
        }
        paged.extend(names(&result.cards));
        page = page.next();
    }
    assert_eq!(paged, whole);
}
