//! Goal: search by scope, name fragment and type line behaves over the recorded card fixture, including accent folding, literal wildcards and hostile input.

use crate::common;

use mtg_db::{CardSearch, Page};
use sqlx::PgPool;

use common::search::{ALL_DECK_CARDS, Case, found, named, run, types, with};
use common::{repo, seeded};

/// Names that keep a `//` once normalized (a reversible card repeating its name does not).
const MULTI_FACED: [&str; 4] = [
    "Bonecrusher Giant // Stomp",
    "Delver of Secrets // Insectile Aberration",
    "Fire // Ice",
    "Valakut Awakening // Valakut Stoneforge",
];

fn scope_and_name_cases() -> Vec<Case> {
    vec![
        found(
            "no criteria returns deck cards only",
            CardSearch::default(),
            &ALL_DECK_CARDS,
        ),
        found(
            "non-deck objects on request",
            with(|s| s.include_non_deck = true),
            &[
                &ALL_DECK_CARDS[..],
                &[
                    "Goblin",
                    "Lightning Bolt // Lightning Bolt",
                    "Elspeth, Sun's Champion Emblem",
                ],
            ]
            .concat(),
        ),
        found("name fragment", named("bolt"), &["Lightning Bolt"]),
        found("name ignores case", named("BoLt"), &["Lightning Bolt"]),
        found(
            "name fragment with non-deck objects",
            with(|s| {
                s.name = Some("bolt".into());
                s.include_non_deck = true;
            }),
            &["Lightning Bolt", "Lightning Bolt // Lightning Bolt"],
        ),
        found(
            "accent-free spelling finds the accented name",
            named("aether"),
            &["Æther Vial"],
        ),
        found(
            "ligature spelling finds it too",
            named("æther"),
            &["Æther Vial"],
        ),
        found(
            "diacritics in the card are folded",
            named("lim-dul"),
            &["Lim-Dûl's Vault"],
        ),
        found(
            "face separator matches cards with different faces",
            named("//"),
            &MULTI_FACED,
        ),
        found("percent is literal", named("100%"), &[]),
        found("underscore is literal", named("_"), &[]),
        found("backslash is literal", named("\\"), &[]),
        found("blank name is ignored", named("   "), &ALL_DECK_CARDS),
    ]
}

fn type_cases() -> Vec<Case> {
    vec![
        found(
            "creature",
            types(&["creature"]),
            &[
                "Atraxa, Praetors' Voice",
                "Bonecrusher Giant // Stomp",
                "Delver of Secrets // Insectile Aberration",
                "Little Girl",
            ],
        ),
        found(
            "every term must match",
            types(&["legendary", "creature"]),
            &["Atraxa, Praetors' Voice"],
        ),
        found(
            "planeswalker",
            types(&["planeswalker"]),
            &[
                "Elspeth, Sun's Champion // Elspeth, Sun's Champion",
                "Jace, the Mind Sculptor",
            ],
        ),
        found(
            "case is ignored",
            types(&["INSECT"]),
            &["Delver of Secrets // Insectile Aberration"],
        ),
        found(
            "back-face types count",
            types(&["adventure"]),
            &["Bonecrusher Giant // Stomp"],
        ),
        found(
            "instant",
            types(&["instant"]),
            &[
                "Bonecrusher Giant // Stomp",
                "Fire // Ice",
                "Lightning Bolt",
                "Lim-Dûl's Vault",
                "Pyroblast",
                "Shock",
                "Valakut Awakening // Valakut Stoneforge",
            ],
        ),
        found(
            "land",
            types(&["land"]),
            &[
                "Forest",
                "Island",
                "Mountain",
                "Valakut Awakening // Valakut Stoneforge",
            ],
        ),
        found(
            "basic land",
            types(&["basic", "land"]),
            &["Forest", "Island", "Mountain"],
        ),
        found(
            "artifact equipment",
            types(&["artifact", "equipment"]),
            &["Hazmat Suit (Used)"],
        ),
        found(
            "subtype only",
            types(&["equipment"]),
            &["Hazmat Suit (Used)"],
        ),
        found(
            "terms in different faces",
            types(&["creature", "land"]),
            &[],
        ),
        found("percent is literal", types(&["%"]), &[]),
        found("underscore is literal", types(&["_"]), &[]),
        found(
            "blank terms are ignored",
            types(&["", "  "]),
            &ALL_DECK_CARDS,
        ),
        found("token type needs non-deck scope", types(&["token"]), &[]),
        found(
            "token type with non-deck scope",
            with(|s| {
                s.type_terms = vec!["token".into()];
                s.include_non_deck = true;
            }),
            &["Goblin"],
        ),
    ]
}

#[sqlx::test]
async fn scope_and_name_filters(pool: PgPool) {
    seeded(pool.clone()).await;
    run(&repo(&pool), scope_and_name_cases()).await;
}

#[sqlx::test]
async fn type_line_filters(pool: PgPool) {
    seeded(pool.clone()).await;
    run(&repo(&pool), type_cases()).await;
}

#[sqlx::test]
async fn searching_after_hostile_input_still_works(pool: PgPool) {
    seeded(pool.clone()).await;
    let repo = repo(&pool);
    let hostile = CardSearch {
        name: Some("'; DROP TABLE cards; --".into()),
        type_terms: vec!["\" OR 1=1 --".into()],
        oracle_text: Some("x') OR ('1'='1".into()),
        legal_in: Some("commander'; DELETE FROM cards; --".into()),
        ..CardSearch::default()
    };
    assert_eq!(
        repo.search(&hostile, Page::default()).await.unwrap().total,
        0
    );
    let all = repo
        .search(&CardSearch::default(), Page::default())
        .await
        .unwrap();
    assert_eq!(all.total, 20);
}
