//! Goal: exact name lookup is forgiving only in ways that cannot confuse two cards, and fuzzy suggestions rank exact, prefix and typo matches sensibly, including single faces of multi-faced cards.

mod common;

use mtg_db::cards::MAX_SUGGESTIONS;
use sqlx::PgPool;

use common::{another_card, db, names, oracle_of, record, repo, seeded};

/// `(query, cards it must find)`; one card means a match, none unknown.
const LOOKUPS: &[(&str, &[&str])] = &[
    ("Lightning Bolt", &["Lightning Bolt"]),
    ("lightning bolt", &["Lightning Bolt"]),
    ("  LIGHTNING   BOLT  ", &["Lightning Bolt"]),
    ("Aether Vial", &["Æther Vial"]),
    ("Æther Vial", &["Æther Vial"]),
    ("Lim-Dul's Vault", &["Lim-Dûl's Vault"]),
    ("Lim-Dûl’s Vault", &["Lim-Dûl's Vault"]),
    ("Atraxa, Praetors’ Voice", &["Atraxa, Praetors' Voice"]),
    ("Fire // Ice", &["Fire // Ice"]),
    ("Fire/Ice", &["Fire // Ice"]),
    ("fire//ice", &["Fire // Ice"]),
    ("Fire", &["Fire // Ice"]),
    (
        "Delver of Secrets",
        &["Delver of Secrets // Insectile Aberration"],
    ),
    (
        "Elspeth, Sun's Champion",
        &["Elspeth, Sun's Champion // Elspeth, Sun's Champion"],
    ),
    ("Ice", &[]),
    ("Insectile Aberration", &[]),
    ("Goblin", &[]),
    ("Lightning Bolt // Lightning Bolt", &["Lightning Bolt"]),
    ("Lightnin Bolt", &[]),
    ("Bolt", &[]),
    ("", &[]),
    ("   ", &[]),
];

#[sqlx::test]
async fn names_resolve_exactly_after_normalization(pool: PgPool) {
    seeded(pool.clone()).await;
    let repo = repo(&pool);
    let mut failures = Vec::new();
    for (query, expected) in LOOKUPS {
        let found = names(&repo.find_by_name(query).await.unwrap());
        if found != *expected {
            failures.push(format!("{query:?}: expected {expected:?}, got {found:?}"));
        }
    }
    assert!(failures.is_empty(), "{failures:#?}");
}

#[sqlx::test]
async fn lookups_agree_with_the_in_memory_index(pool: PgPool) {
    seeded(pool.clone()).await;
    let repo = repo(&pool);
    let index = mtg_import::resolve::CardIndex::from_records(&common::fixture_records());
    for (query, _) in LOOKUPS {
        let in_memory: Vec<String> = match index.find(query) {
            mtg_import::resolve::Lookup::Found(card) => vec![card.name.clone()],
            mtg_import::resolve::Lookup::Ambiguous(cards) => {
                cards.iter().map(|c| c.name.clone()).collect()
            }
            mtg_import::resolve::Lookup::Missing => Vec::new(),
        };
        assert_eq!(
            names(&repo.find_by_name(query).await.unwrap()),
            in_memory,
            "{query:?}"
        );
    }
}

#[sqlx::test]
async fn shared_names_are_reported_not_guessed(pool: PgPool) {
    let database = seeded(pool).await;
    let base = record("Shock");
    database
        .importer()
        .import_cards(
            [
                another_card(&base, 80, "Twin"),
                another_card(&base, 81, "twin"),
            ]
            .into_iter()
            .map(Ok),
        )
        .await
        .unwrap();

    let found = database.cards().find_by_name("TWIN").await.unwrap();

    assert_eq!(found.len(), 2);
    assert_ne!(found[0].oracle_id, found[1].oracle_id);
}

#[sqlx::test]
async fn a_full_name_match_beats_a_front_face_match(pool: PgPool) {
    let database = db(pool);
    let base = record("Shock");
    database
        .importer()
        .import_cards(
            [
                another_card(&base, 70, "Dusk"),
                another_card(&base, 71, "Dusk // Dawn"),
                another_card(&base, 72, "Dusk // Night"),
            ]
            .into_iter()
            .map(Ok),
        )
        .await
        .unwrap();
    let repo = database.cards();

    assert_eq!(names(&repo.find_by_name("Dusk").await.unwrap()), ["Dusk"]);
    assert_eq!(
        names(&repo.find_by_name("Dusk // Dawn").await.unwrap()),
        ["Dusk // Dawn"]
    );
    assert!(repo.find_by_name("Dawn").await.unwrap().is_empty());
}

/// `(query, the card that must rank first)`.
const TYPOS: &[(&str, &str)] = &[
    ("lightnin bolt", "Lightning Bolt"),
    ("lightening bolt", "Lightning Bolt"),
    ("lightning blot", "Lightning Bolt"),
    ("Lightning Bolt", "Lightning Bolt"),
    ("sol rng", "Sol Ring"),
    ("jace the mind sculptor", "Jace, the Mind Sculptor"),
    ("jace mind sculpter", "Jace, the Mind Sculptor"),
    ("atraxa praetors voice", "Atraxa, Praetors' Voice"),
    ("aether vial", "Æther Vial"),
    ("arcane signet", "Arcane Signet"),
    ("arcain signet", "Arcane Signet"),
    ("pyroblat", "Pyroblast"),
    ("bonecrusher giant", "Bonecrusher Giant // Stomp"),
    (
        "delvr of secrets",
        "Delver of Secrets // Insectile Aberration",
    ),
    (
        "Insectile Aberration",
        "Delver of Secrets // Insectile Aberration",
    ),
    ("stomp", "Bonecrusher Giant // Stomp"),
    ("fire", "Fire // Ice"),
    ("ice", "Fire // Ice"),
    (
        "valakut stoneforge",
        "Valakut Awakening // Valakut Stoneforge",
    ),
    (
        "elspeth sun's champion",
        "Elspeth, Sun's Champion // Elspeth, Sun's Champion",
    ),
];

#[sqlx::test]
async fn typos_and_partial_names_find_their_card_first(pool: PgPool) {
    seeded(pool.clone()).await;
    let repo = repo(&pool);
    let mut failures = Vec::new();
    for (query, expected) in TYPOS {
        let suggestions = repo.suggest(query, 5).await.unwrap();
        let first = suggestions.first().map(|s| s.name.as_str());
        if first != Some(*expected) {
            failures.push(format!(
                "{query:?}: expected {expected:?} first, got {suggestions:?}"
            ));
        }
    }
    assert!(failures.is_empty(), "{failures:#?}");
}

#[sqlx::test]
async fn suggestions_carry_ids_and_similarities_in_range(pool: PgPool) {
    seeded(pool.clone()).await;
    let suggestions = repo(&pool).suggest("lightnin bolt", 5).await.unwrap();

    assert_eq!(suggestions[0].oracle_id, oracle_of("Lightning Bolt"));
    assert!(
        suggestions
            .iter()
            .all(|s| (0.0..=1.0).contains(&s.similarity))
    );
    let exact = repo(&pool).suggest("Shock", 1).await.unwrap();
    assert!((exact[0].similarity - 1.0).abs() < 1e-6);
}

#[sqlx::test]
async fn nothing_is_suggested_for_gibberish_blanks_or_non_deck_objects(pool: PgPool) {
    seeded(pool.clone()).await;
    let repo = repo(&pool);
    for query in ["zzzzzzzz", "qxqxqxqx", "", "   ", "!!!", "\u{200B}"] {
        assert!(
            repo.suggest(query, 5).await.unwrap().is_empty(),
            "{query:?}"
        );
    }
    assert!(
        repo.suggest("goblin", 5).await.unwrap().is_empty(),
        "tokens are not deck cards"
    );
}

/// Synthetic names around one stem, so ranking among near neighbors is visible.
const LIGHTNING: [&str; 7] = [
    "Lightning Bolt",
    "Lightning Helix",
    "Lightning Greaves",
    "Lightning Strike",
    "Lightning Bolt Tutor",
    "Chain Lightning",
    "Enlightened Tutor",
];

async fn with_lightning_family(pool: PgPool) -> mtg_db::CardRepo {
    let database = db(pool);
    let base = record("Shock");
    let cards = LIGHTNING
        .iter()
        .enumerate()
        .map(|(n, name)| Ok(another_card(&base, 300 + n as u128, name)));
    database.importer().import_cards(cards).await.unwrap();
    database.cards()
}

#[sqlx::test]
async fn exact_beats_prefix_beats_similar(pool: PgPool) {
    let repo = with_lightning_family(pool).await;

    let exact: Vec<_> = repo
        .suggest("lightning bolt", 10)
        .await
        .unwrap()
        .into_iter()
        .map(|s| s.name)
        .collect();
    assert_eq!(exact[0], "Lightning Bolt", "exact match first");
    assert_eq!(
        exact[1], "Lightning Bolt Tutor",
        "prefix match before merely similar names"
    );

    let prefix: Vec<_> = repo
        .suggest("lightning", 10)
        .await
        .unwrap()
        .into_iter()
        .map(|s| s.name)
        .collect();
    let first_five: std::collections::BTreeSet<_> =
        prefix[..5].iter().map(String::as_str).collect();
    assert!(
        [
            "Lightning Bolt",
            "Lightning Greaves",
            "Lightning Helix",
            "Lightning Strike",
            "Lightning Bolt Tutor"
        ]
        .iter()
        .all(|name| first_five.contains(name)),
        "every name starting with the query comes first: {prefix:?}"
    );
}

#[sqlx::test]
async fn closer_typos_rank_above_farther_ones(pool: PgPool) {
    let repo = with_lightning_family(pool).await;

    let helix: Vec<_> = repo
        .suggest("lightnin helx", 10)
        .await
        .unwrap()
        .into_iter()
        .map(|s| s.name)
        .collect();

    assert_eq!(helix[0], "Lightning Helix");
    let scores = repo.suggest("lightnin helx", 10).await.unwrap();
    assert!(
        scores
            .windows(2)
            .all(|pair| pair[0].similarity >= pair[1].similarity - 1e-6),
        "similarity never increases down the list: {scores:?}"
    );
}

#[sqlx::test]
async fn suggestion_limits_are_respected_and_clamped(pool: PgPool) {
    let repo = with_lightning_family(pool).await;

    assert_eq!(repo.suggest("lightning", 3).await.unwrap().len(), 3);
    assert_eq!(
        repo.suggest("lightning", 0).await.unwrap().len(),
        1,
        "zero is raised to one"
    );
    let many = repo.suggest("lightning", u32::MAX).await.unwrap();
    assert!(many.len() <= MAX_SUGGESTIONS as usize);
    assert!(many.len() >= 6);
}

#[sqlx::test]
async fn equally_good_suggestions_are_ordered_deterministically(pool: PgPool) {
    let database = db(pool);
    let base = record("Shock");
    let cards = ["Alpha Twin", "Alpha Twin", "Alpha Twin"]
        .iter()
        .enumerate()
        .map(|(n, name)| Ok(another_card(&base, 600 + n as u128, name)));
    database.importer().import_cards(cards).await.unwrap();

    let first = database.cards().suggest("alpha twn", 10).await.unwrap();
    let second = database.cards().suggest("alpha twn", 10).await.unwrap();

    assert_eq!(first, second);
    let ids: Vec<_> = first.iter().map(|s| s.oracle_id.clone()).collect();
    let mut sorted = ids.clone();
    sorted.sort();
    assert_eq!(ids, sorted, "ties fall back to the oracle id");
}
