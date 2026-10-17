//! Goal: the color-identity-subset filter matches Commander's deck-construction rule for every commander identity, including colorless commanders, partners and cards whose identity exceeds their colors.

mod common;

use mtg_core::color::{Color, ColorSet};
use mtg_db::{CardSearch, ColorFilter, ColorMatch, Page};
use sqlx::PgPool;

use common::{another_card, colors, db, in_search_order, names, record, repo, seeded};

fn within(identity: ColorSet) -> CardSearch {
    CardSearch {
        color_identity_within: Some(identity),
        ..CardSearch::default()
    }
}

/// Every one of the 32 color sets, as WUBRG letters.
fn all_identities() -> Vec<String> {
    (0u8..32)
        .map(|mask| {
            Color::ALL
                .iter()
                .enumerate()
                .filter(|(bit, _)| mask >> bit & 1 == 1)
                .map(|(_, color)| match color {
                    Color::White => 'W',
                    Color::Blue => 'U',
                    Color::Black => 'B',
                    Color::Red => 'R',
                    Color::Green => 'G',
                })
                .collect()
        })
        .collect()
}

const COLORLESS_IDENTITY: [&str; 5] = [
    "Æther Vial",
    "Arcane Signet",
    "Hazmat Suit (Used)",
    "Mystery Card",
    "Sol Ring",
];

/// Fixture decks a commander of the given identity could legally contain.
fn commander_scenarios() -> Vec<(&'static str, &'static str, Vec<&'static str>)> {
    let with_colorless = |extra: &[&'static str]| [&COLORLESS_IDENTITY[..], extra].concat();
    vec![
        ("colorless commander", "", with_colorless(&[])),
        (
            "mono-red",
            "R",
            with_colorless(&[
                "Bonecrusher Giant // Stomp",
                "Lightning Bolt",
                "Mountain",
                "Pyroblast",
                "Shock",
                "Valakut Awakening // Valakut Stoneforge",
            ]),
        ),
        ("mono-green", "G", with_colorless(&["Forest"])),
        (
            "mono-white",
            "W",
            with_colorless(&[
                "Elspeth, Sun's Champion // Elspeth, Sun's Champion",
                "Little Girl",
            ]),
        ),
        (
            "Izzet",
            "UR",
            with_colorless(&[
                "Bonecrusher Giant // Stomp",
                "Delver of Secrets // Insectile Aberration",
                "Fire // Ice",
                "Island",
                "Jace, the Mind Sculptor",
                "Lightning Bolt",
                "Mountain",
                "Pyroblast",
                "Shock",
                "Valakut Awakening // Valakut Stoneforge",
            ]),
        ),
        (
            "Atraxa's four colors",
            "WUBG",
            with_colorless(&[
                "Atraxa, Praetors' Voice",
                "Delver of Secrets // Insectile Aberration",
                "Elspeth, Sun's Champion // Elspeth, Sun's Champion",
                "Forest",
                "Island",
                "Jace, the Mind Sculptor",
                "Lim-Dûl's Vault",
                "Little Girl",
            ]),
        ),
    ]
}

#[sqlx::test]
async fn commanders_get_exactly_the_cards_inside_their_identity(pool: PgPool) {
    seeded(pool.clone()).await;
    let repo = repo(&pool);
    let mut failures = Vec::new();
    for (label, identity, expected) in commander_scenarios() {
        let page = repo
            .search(&within(colors(identity)), Page::new(200, 0))
            .await
            .unwrap();
        if names(&page.cards) != in_search_order(&expected) {
            failures.push(format!("{label}: got {:?}", names(&page.cards)));
        }
    }
    assert!(failures.is_empty(), "{failures:#?}");

    let everything = repo
        .search(&within(colors("WUBRG")), Page::new(200, 0))
        .await
        .unwrap();
    assert_eq!(
        everything.total, 20,
        "a five-color commander allows every deck card"
    );
}

#[sqlx::test]
async fn partner_commanders_combine_their_identities(pool: PgPool) {
    seeded(pool.clone()).await;
    let repo = repo(&pool);
    let partners = [
        ("W", "R", "WR"),
        ("U", "U", "U"),
        ("", "G", "G"),
        ("", "", ""),
        ("WU", "BR", "WUBR"),
    ];
    for (first, second, expected) in partners {
        let combined = union(colors(first), colors(second));
        assert_eq!(combined, colors(expected), "{first} + {second}");
        let page = repo
            .search(&within(combined), Page::new(200, 0))
            .await
            .unwrap();
        let alone = repo
            .search(&within(colors(first)), Page::new(200, 0))
            .await
            .unwrap();
        assert!(
            page.total >= alone.total,
            "{first} + {second} never allows less than {first}"
        );
    }
    let white_red = repo
        .search(&within(colors("WR")), Page::new(200, 0))
        .await
        .unwrap();
    let cards = names(&white_red.cards);
    assert!(cards.contains(&"Little Girl".to_owned()) && cards.contains(&"Mountain".to_owned()));
    assert!(!cards.contains(&"Island".to_owned()));
}

/// Colors in either set: the identity a pair of partner commanders shares.
fn union(a: ColorSet, b: ColorSet) -> ColorSet {
    Color::ALL
        .into_iter()
        .filter(|&color| a.contains(color) || b.contains(color))
        .collect()
}

/// One synthetic card per possible identity, so every commander can be checked exhaustively.
async fn one_card_per_identity(pool: &PgPool) -> Vec<(String, ColorSet)> {
    let base = record("Shock");
    let identities = all_identities();
    let records: Vec<_> = identities
        .iter()
        .enumerate()
        .map(|(n, symbols)| {
            let mut card = another_card(&base, 500 + n as u128, &format!("Identity {symbols:?}"));
            card.color_identity = colors(symbols);
            card.colors = ColorSet::EMPTY;
            card
        })
        .collect();
    db(pool.clone())
        .importer()
        .import_cards(records.into_iter().map(Ok))
        .await
        .unwrap_or_else(|e| panic!("identity cards failed to import: {e}"));
    identities
        .into_iter()
        .map(|symbols| (format!("Identity {symbols:?}"), colors(&symbols)))
        .collect()
}

#[sqlx::test]
async fn sql_and_rust_agree_for_every_commander_and_every_identity(pool: PgPool) {
    let cards = one_card_per_identity(&pool).await;
    let repo = repo(&pool);
    for commander in all_identities() {
        let commander_set = colors(&commander);
        let page = repo
            .search(&within(commander_set), Page::new(200, 0))
            .await
            .unwrap();
        let mut expected: Vec<String> = cards
            .iter()
            .filter(|(_, identity)| identity.is_subset_of(commander_set))
            .map(|(name, _)| name.clone())
            .collect();
        expected.sort_by_key(|name| mtg_import::resolve::normalize_name(name));
        assert_eq!(names(&page.cards), expected, "commander {commander:?}");
        assert_eq!(
            page.total,
            1 << commander_set.len(),
            "a {}-color commander allows 2^{} identities",
            commander_set.len(),
            commander_set.len()
        );
    }
}

#[sqlx::test]
async fn identity_can_exceed_a_cards_colors(pool: PgPool) {
    // A red card with a green activated ability: its colors are R but its identity is RG.
    let mut off_color = another_card(&record("Shock"), 900, "Red Card With Green Ability");
    off_color.colors = colors("R");
    off_color.color_identity = colors("RG");
    // A colorless artifact whose rules text mentions blue mana.
    let mut colorless = another_card(&record("Sol Ring"), 901, "Colorless With Blue Ability");
    colorless.colors = ColorSet::EMPTY;
    colorless.color_identity = colors("U");
    // A card with a color indicator and no mana cost: colored, identity follows the indicator.
    let mut indicator = another_card(&record("Shock"), 902, "Indicator Card");
    indicator.mana_cost = String::new();
    indicator.colors = colors("B");
    indicator.color_identity = colors("B");
    let database = db(pool.clone());
    database
        .importer()
        .import_cards([off_color, colorless, indicator].into_iter().map(Ok))
        .await
        .unwrap();
    let repo = repo(&pool);
    let search = |search: CardSearch| {
        let repo = repo.clone();
        async move { names(&repo.search(&search, Page::default()).await.unwrap().cards) }
    };

    let by_colors = search(CardSearch {
        colors: Some(ColorFilter {
            colors: colors("R"),
            mode: ColorMatch::Within,
        }),
        ..CardSearch::default()
    })
    .await;
    assert!(by_colors.contains(&"Red Card With Green Ability".to_owned()));
    assert!(by_colors.contains(&"Colorless With Blue Ability".to_owned()));

    let by_identity = search(within(colors("R"))).await;
    assert!(!by_identity.contains(&"Red Card With Green Ability".to_owned()));
    assert!(!by_identity.contains(&"Colorless With Blue Ability".to_owned()));
    assert!(!by_identity.contains(&"Indicator Card".to_owned()));

    let red_green = search(within(colors("RG"))).await;
    assert!(red_green.contains(&"Red Card With Green Ability".to_owned()));
    assert!(!red_green.contains(&"Colorless With Blue Ability".to_owned()));
    let black = search(within(colors("B"))).await;
    assert_eq!(black.iter().filter(|n| *n == "Indicator Card").count(), 1);
}

#[sqlx::test]
async fn identity_filter_combines_with_the_other_filters(pool: PgPool) {
    seeded(pool.clone()).await;
    let repo = repo(&pool);
    let page = repo
        .search(
            &CardSearch {
                color_identity_within: Some(colors("UR")),
                type_terms: vec!["land".into()],
                ..CardSearch::default()
            },
            Page::default(),
        )
        .await
        .unwrap();

    assert_eq!(
        names(&page.cards),
        [
            "Island",
            "Mountain",
            "Valakut Awakening // Valakut Stoneforge"
        ]
    );
}
