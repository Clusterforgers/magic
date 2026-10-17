//! Goal: every search filter works on its own and in combination, with blank and hostile input handled safely, over the recorded card fixture.

mod common;

use mtg_db::{CardRepo, CardSearch, ColorFilter, ColorMatch, Page, SortKey};
use sqlx::PgPool;

use common::{colors, in_search_order, names, repo, seeded};

struct Case {
    label: &'static str,
    search: CardSearch,
    expected: Vec<String>,
}

/// A case whose results are the named cards in default (name) order.
fn found(label: &'static str, search: CardSearch, names: &[&str]) -> Case {
    Case {
        label,
        search,
        expected: in_search_order(names),
    }
}

/// A case whose results must come back in exactly the given order.
fn ordered(label: &'static str, search: CardSearch, names: &[&str]) -> Case {
    Case {
        label,
        search,
        expected: names.iter().map(|&n| n.to_owned()).collect(),
    }
}

fn with(f: impl FnOnce(&mut CardSearch)) -> CardSearch {
    let mut search = CardSearch::default();
    f(&mut search);
    search
}

fn colors_are(symbols: &str, mode: ColorMatch) -> CardSearch {
    with(|s| {
        s.colors = Some(ColorFilter {
            colors: colors(symbols),
            mode,
        });
    })
}

fn mana(min: Option<f64>, max: Option<f64>) -> CardSearch {
    with(|s| {
        s.min_mana_value = min;
        s.max_mana_value = max;
    })
}

fn types(terms: &[&str]) -> CardSearch {
    with(|s| s.type_terms = terms.iter().map(|&t| t.to_owned()).collect())
}

fn text(query: &str) -> CardSearch {
    with(|s| s.oracle_text = Some(query.to_owned()))
}

fn legal(format: &str) -> CardSearch {
    with(|s| s.legal_in = Some(format.to_owned()))
}

fn named(fragment: &str) -> CardSearch {
    with(|s| s.name = Some(fragment.to_owned()))
}

const ALL_DECK_CARDS: [&str; 20] = [
    "Æther Vial",
    "Arcane Signet",
    "Atraxa, Praetors' Voice",
    "Bonecrusher Giant // Stomp",
    "Delver of Secrets // Insectile Aberration",
    "Elspeth, Sun's Champion // Elspeth, Sun's Champion",
    "Fire // Ice",
    "Forest",
    "Hazmat Suit (Used)",
    "Island",
    "Jace, the Mind Sculptor",
    "Lightning Bolt",
    "Lim-Dûl's Vault",
    "Little Girl",
    "Mountain",
    "Mystery Card",
    "Pyroblast",
    "Shock",
    "Sol Ring",
    "Valakut Awakening // Valakut Stoneforge",
];

const COLORLESS: [&str; 8] = [
    "Æther Vial",
    "Arcane Signet",
    "Forest",
    "Hazmat Suit (Used)",
    "Island",
    "Mountain",
    "Mystery Card",
    "Sol Ring",
];

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

fn mana_value_cases() -> Vec<Case> {
    vec![
        found(
            "at least four",
            mana(Some(4.0), None),
            &[
                "Atraxa, Praetors' Voice",
                "Elspeth, Sun's Champion // Elspeth, Sun's Champion",
                "Fire // Ice",
                "Jace, the Mind Sculptor",
            ],
        ),
        found(
            "at most zero",
            mana(None, Some(0.0)),
            &["Forest", "Island", "Mountain", "Mystery Card"],
        ),
        found("exactly half", mana(Some(0.5), Some(0.5)), &["Little Girl"]),
        found(
            "exactly one",
            mana(Some(1.0), Some(1.0)),
            &[
                "Æther Vial",
                "Delver of Secrets // Insectile Aberration",
                "Lightning Bolt",
                "Pyroblast",
                "Shock",
                "Sol Ring",
            ],
        ),
        found(
            "two to three inclusive",
            mana(Some(2.0), Some(3.0)),
            &[
                "Arcane Signet",
                "Bonecrusher Giant // Stomp",
                "Hazmat Suit (Used)",
                "Lim-Dûl's Vault",
                "Valakut Awakening // Valakut Stoneforge",
            ],
        ),
        found(
            "fractions below the bound",
            mana(None, Some(0.4)),
            &["Forest", "Island", "Mountain", "Mystery Card"],
        ),
        found("above everything", mana(Some(7.0), None), &[]),
        found(
            "inverted range matches nothing",
            mana(Some(5.0), Some(3.0)),
            &[],
        ),
        found(
            "not-a-number bounds are ignored",
            mana(Some(f64::NAN), Some(f64::NAN)),
            &ALL_DECK_CARDS,
        ),
        found(
            "infinite bounds are open",
            mana(Some(f64::NEG_INFINITY), Some(f64::INFINITY)),
            &ALL_DECK_CARDS,
        ),
    ]
}

fn color_cases() -> Vec<Case> {
    use ColorMatch::{Exactly, Includes, Overlaps, Within};
    let red = &[
        "Bonecrusher Giant // Stomp",
        "Lightning Bolt",
        "Pyroblast",
        "Shock",
        "Valakut Awakening // Valakut Stoneforge",
    ];
    vec![
        found("exactly red", colors_are("R", Exactly), red),
        found("exactly colorless", colors_are("", Exactly), &COLORLESS),
        found(
            "exactly two colors",
            colors_are("UR", Exactly),
            &["Fire // Ice"],
        ),
        found(
            "exactly blue",
            colors_are("U", Exactly),
            &[
                "Delver of Secrets // Insectile Aberration",
                "Jace, the Mind Sculptor",
            ],
        ),
        found(
            "exactly four colors",
            colors_are("WUBG", Exactly),
            &["Atraxa, Praetors' Voice"],
        ),
        found("exactly all five", colors_are("WUBRG", Exactly), &[]),
        found(
            "includes blue",
            colors_are("U", Includes),
            &[
                "Atraxa, Praetors' Voice",
                "Delver of Secrets // Insectile Aberration",
                "Fire // Ice",
                "Jace, the Mind Sculptor",
                "Lim-Dûl's Vault",
            ],
        ),
        found(
            "includes blue and black",
            colors_are("UB", Includes),
            &["Atraxa, Praetors' Voice", "Lim-Dûl's Vault"],
        ),
        found(
            "includes nothing is everything",
            colors_are("", Includes),
            &ALL_DECK_CARDS,
        ),
        found("includes all five", colors_are("WUBRG", Includes), &[]),
        found(
            "within blue-red",
            colors_are("UR", Within),
            &[
                &COLORLESS[..],
                &[
                    "Delver of Secrets // Insectile Aberration",
                    "Jace, the Mind Sculptor",
                    "Fire // Ice",
                ],
                red,
            ]
            .concat(),
        ),
        found(
            "within nothing is colorless",
            colors_are("", Within),
            &COLORLESS,
        ),
        found(
            "within all five is everything",
            colors_are("WUBRG", Within),
            &ALL_DECK_CARDS,
        ),
        found(
            "overlaps white-black",
            colors_are("WB", Overlaps),
            &[
                "Atraxa, Praetors' Voice",
                "Elspeth, Sun's Champion // Elspeth, Sun's Champion",
                "Lim-Dûl's Vault",
                "Little Girl",
            ],
        ),
        found(
            "overlaps red",
            colors_are("R", Overlaps),
            &[&red[..], &["Fire // Ice"]].concat(),
        ),
        found(
            "overlaps nothing matches nothing",
            colors_are("", Overlaps),
            &[],
        ),
    ]
}

fn legality_cases() -> Vec<Case> {
    vec![
        found(
            "commander",
            legal("commander"),
            &[
                "Æther Vial",
                "Arcane Signet",
                "Atraxa, Praetors' Voice",
                "Bonecrusher Giant // Stomp",
                "Delver of Secrets // Insectile Aberration",
                "Fire // Ice",
                "Forest",
                "Island",
                "Jace, the Mind Sculptor",
                "Lightning Bolt",
                "Lim-Dûl's Vault",
                "Mountain",
                "Pyroblast",
                "Shock",
                "Sol Ring",
                "Valakut Awakening // Valakut Stoneforge",
            ],
        ),
        found(
            "format names ignore case and padding",
            legal("  CoMmAnDeR "),
            &[
                "Æther Vial",
                "Arcane Signet",
                "Atraxa, Praetors' Voice",
                "Bonecrusher Giant // Stomp",
                "Delver of Secrets // Insectile Aberration",
                "Fire // Ice",
                "Forest",
                "Island",
                "Jace, the Mind Sculptor",
                "Lightning Bolt",
                "Lim-Dûl's Vault",
                "Mountain",
                "Pyroblast",
                "Shock",
                "Sol Ring",
                "Valakut Awakening // Valakut Stoneforge",
            ],
        ),
        found(
            "modern",
            legal("modern"),
            &[
                "Æther Vial",
                "Bonecrusher Giant // Stomp",
                "Delver of Secrets // Insectile Aberration",
                "Fire // Ice",
                "Forest",
                "Island",
                "Lightning Bolt",
                "Mountain",
                "Pyroblast",
                "Shock",
                "Valakut Awakening // Valakut Stoneforge",
            ],
        ),
        found(
            "standard",
            legal("standard"),
            &["Forest", "Island", "Mountain"],
        ),
        found(
            "restricted counts as legal",
            legal("vintage"),
            &["Jace, the Mind Sculptor", "Lightning Bolt", "Sol Ring"],
        ),
        found(
            "banned does not",
            legal("legacy"),
            &[
                "Æther Vial",
                "Fire // Ice",
                "Lightning Bolt",
                "Lim-Dûl's Vault",
            ],
        ),
        found("unknown format", legal("no-such-format"), &[]),
        found("blank format is ignored", legal(" "), &ALL_DECK_CARDS),
    ]
}

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

async fn run(repo: &CardRepo, cases: Vec<Case>) {
    let mut failures = Vec::new();
    for case in cases {
        let page = repo
            .search(&case.search, Page::new(200, 0))
            .await
            .unwrap_or_else(|e| panic!("{}: {e}", case.label));
        let actual = names(&page.cards);
        if actual != case.expected || page.total != case.expected.len() as u64 {
            failures.push(format!(
                "{}:\n  expected {:?}\n  actual   {:?} (total {})",
                case.label, case.expected, actual, page.total
            ));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
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
async fn mana_value_filters(pool: PgPool) {
    seeded(pool.clone()).await;
    run(&repo(&pool), mana_value_cases()).await;
}

#[sqlx::test]
async fn color_filters(pool: PgPool) {
    seeded(pool.clone()).await;
    run(&repo(&pool), color_cases()).await;
}

#[sqlx::test]
async fn legality_filters(pool: PgPool) {
    seeded(pool.clone()).await;
    run(&repo(&pool), legality_cases()).await;
}

#[sqlx::test]
async fn oracle_text_filters(pool: PgPool) {
    seeded(pool.clone()).await;
    run(&repo(&pool), text_cases()).await;
}

#[sqlx::test]
async fn combined_filters_and_sorting(pool: PgPool) {
    seeded(pool.clone()).await;
    let mut cases = combined_cases();
    cases.extend(sort_cases());
    run(&repo(&pool), cases).await;
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
