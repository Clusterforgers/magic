//! Goal: oracle-text full-text search, combinations of every filter, and the mana-value sort order behave over the recorded card fixture.

use crate::common;

use mtg_db::{CardSearch, ColorFilter, ColorMatch, SortKey};
use sqlx::PgPool;

use common::search::{ALL_DECK_CARDS, Case, found, ordered, run, text, types, with};
use common::{colors, repo, seeded};

fn text_cases() -> Vec<Case> {
    vec![
        found(
            "single word",
            text("lifelink"),
            &["Atraxa, Praetors' Voice"],
        ),
        found(
            "words are stemmed",
            text("creatures"),
            &[
                "Æther Vial",
                "Bonecrusher Giant // Stomp",
                "Hazmat Suit (Used)",
                "Jace, the Mind Sculptor",
            ],
        ),
        found(
            "singular finds the same",
            text("creature"),
            &[
                "Æther Vial",
                "Bonecrusher Giant // Stomp",
                "Hazmat Suit (Used)",
                "Jace, the Mind Sculptor",
            ],
        ),
        found(
            "all words must match",
            text("flying vigilance"),
            &["Atraxa, Praetors' Voice"],
        ),
        found(
            "or widens the match",
            text("vigilance or upkeep"),
            &[
                "Æther Vial",
                "Atraxa, Praetors' Voice",
                "Delver of Secrets // Insectile Aberration",
            ],
        ),
        found(
            "minus excludes",
            text("upkeep -charge"),
            &["Delver of Secrets // Insectile Aberration"],
        ),
        found(
            "back-face text is searchable",
            text("flying"),
            &[
                "Atraxa, Praetors' Voice",
                "Delver of Secrets // Insectile Aberration",
            ],
        ),
        found("phrase", text("\"tap target permanent\""), &["Fire // Ice"]),
        found(
            "shorter phrase",
            text("\"target permanent\""),
            &["Fire // Ice", "Pyroblast"],
        ),
        found(
            "damage",
            text("damage"),
            &[
                "Bonecrusher Giant // Stomp",
                "Fire // Ice",
                "Lightning Bolt",
                "Shock",
            ],
        ),
        found(
            "draw",
            text("draw"),
            &[
                "Fire // Ice",
                "Jace, the Mind Sculptor",
                "Valakut Awakening // Valakut Stoneforge",
            ],
        ),
    ]
}

/// Queries that match nothing, are ignored, or try to break out of the query.
fn text_edge_cases() -> Vec<Case> {
    vec![
        found("only stop words matches nothing", text("the"), &[]),
        found("blank query is ignored", text("  "), &ALL_DECK_CARDS),
        found(
            "injection attempts are just words",
            text("'; DROP TABLE cards; --"),
            &[],
        ),
        found(
            "unbalanced quotes are tolerated",
            text("\"damage"),
            &[
                "Bonecrusher Giant // Stomp",
                "Fire // Ice",
                "Lightning Bolt",
                "Shock",
            ],
        ),
        found(
            "non-deck objects on request",
            with(|s| {
                s.oracle_text = Some("creatures".into());
                s.include_non_deck = true;
            }),
            &[
                "Æther Vial",
                "Bonecrusher Giant // Stomp",
                "Hazmat Suit (Used)",
                "Jace, the Mind Sculptor",
                "Elspeth, Sun's Champion Emblem",
            ],
        ),
    ]
}

fn combined_cases() -> Vec<Case> {
    vec![
        found(
            "cheap red-blue instants legal in commander",
            with(|s| {
                s.color_identity_within = Some(colors("UR"));
                s.max_mana_value = Some(2.0);
                s.type_terms = vec!["instant".into()];
                s.legal_in = Some("commander".into());
            }),
            &[
                "Lightning Bolt",
                "Pyroblast",
                "Shock",
                "Valakut Awakening // Valakut Stoneforge",
            ],
        ),
        found(
            "blue cards worth four or more",
            with(|s| {
                s.colors = Some(ColorFilter {
                    colors: colors("U"),
                    mode: ColorMatch::Includes,
                });
                s.min_mana_value = Some(4.0);
            }),
            &[
                "Atraxa, Praetors' Voice",
                "Fire // Ice",
                "Jace, the Mind Sculptor",
            ],
        ),
        found(
            "cheap creatures within white-blue-black-green identity",
            with(|s| {
                s.type_terms = vec!["creature".into()];
                s.color_identity_within = Some(colors("WUBG"));
                s.max_mana_value = Some(3.0);
            }),
            &["Delver of Secrets // Insectile Aberration", "Little Girl"],
        ),
        found(
            "red damage spells legal in modern",
            with(|s| {
                s.colors = Some(ColorFilter {
                    colors: colors("R"),
                    mode: ColorMatch::Exactly,
                });
                s.oracle_text = Some("damage".into());
                s.legal_in = Some("modern".into());
            }),
            &["Bonecrusher Giant // Stomp", "Lightning Bolt", "Shock"],
        ),
        found(
            "every filter at once",
            with(|s| {
                s.name = Some("shock".into());
                s.oracle_text = Some("damage".into());
                s.type_terms = vec!["instant".into()];
                s.colors = Some(ColorFilter {
                    colors: colors("R"),
                    mode: ColorMatch::Includes,
                });
                s.color_identity_within = Some(colors("R"));
                s.min_mana_value = Some(1.0);
                s.max_mana_value = Some(1.0);
                s.legal_in = Some("commander".into());
            }),
            &["Shock"],
        ),
        found(
            "contradictory filters",
            with(|s| {
                s.colors = Some(ColorFilter {
                    colors: colors("R"),
                    mode: ColorMatch::Exactly,
                });
                s.color_identity_within = Some(colors(""));
            }),
            &[],
        ),
    ]
}

fn sort_cases() -> Vec<Case> {
    let by_value = |search: CardSearch| CardSearch {
        sort: SortKey::ManaValue,
        ..search
    };
    vec![
        ordered(
            "mana value ascending, ties by name",
            by_value(CardSearch::default()),
            &[
                "Forest",
                "Island",
                "Mountain",
                "Mystery Card",
                "Little Girl",
                "Æther Vial",
                "Delver of Secrets // Insectile Aberration",
                "Lightning Bolt",
                "Pyroblast",
                "Shock",
                "Sol Ring",
                "Arcane Signet",
                "Hazmat Suit (Used)",
                "Lim-Dûl's Vault",
                "Valakut Awakening // Valakut Stoneforge",
                "Bonecrusher Giant // Stomp",
                "Atraxa, Praetors' Voice",
                "Fire // Ice",
                "Jace, the Mind Sculptor",
                "Elspeth, Sun's Champion // Elspeth, Sun's Champion",
            ],
        ),
        ordered(
            "mana value sort respects filters",
            by_value(types(&["instant"])),
            &[
                "Lightning Bolt",
                "Pyroblast",
                "Shock",
                "Lim-Dûl's Vault",
                "Valakut Awakening // Valakut Stoneforge",
                "Bonecrusher Giant // Stomp",
                "Fire // Ice",
            ],
        ),
    ]
}

#[sqlx::test]
async fn oracle_text_filters(pool: PgPool) {
    seeded(pool.clone()).await;
    let mut cases = text_cases();
    cases.extend(text_edge_cases());
    run(&repo(&pool), cases).await;
}

#[sqlx::test]
async fn combined_filters_and_sorting(pool: PgPool) {
    seeded(pool.clone()).await;
    let mut cases = combined_cases();
    cases.extend(sort_cases());
    run(&repo(&pool), cases).await;
}
