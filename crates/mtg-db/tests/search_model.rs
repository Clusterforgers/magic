//! Goal: for hundreds of random cards and random filter combinations, database search returns exactly what a plain in-memory filter returns, in the same order, and paging through any result never repeats or skips a card.

mod common;

use std::collections::BTreeSet;

use mtg_core::color::{Color, ColorSet};
use mtg_db::cards::MAX_PAGE_SIZE;
use mtg_db::{CardSearch, ColorFilter, ColorMatch, Page, SortKey};
use mtg_import::ids::OracleId;
use mtg_import::resolve::normalize_name;
use mtg_import::scryfall::{CardRecord, Layout, Legality};
use proptest::prelude::*;
use proptest::strategy::ValueTree;
use proptest::test_runner::TestRunner;
use sqlx::PgPool;

use common::{another_card, db, record, repo};

const CARD_COUNT: usize = 250;
const SEARCH_COUNT: usize = 200;

const TYPE_LINES: [&str; 10] = [
    "Creature — Elf Druid",
    "Instant",
    "Sorcery",
    "Artifact — Equipment",
    "Basic Land — Forest",
    "Legendary Planeswalker — Jace",
    "Enchantment — Aura",
    "Artifact Creature — Golem",
    "Land",
    "Legendary Creature — Dragon",
];
const TYPE_TERMS: [&str; 10] = [
    "creature",
    "legendary",
    "land",
    "artifact",
    "instant",
    "elf",
    "golem",
    "—",
    "ARTIFACT",
    "aura",
];
const FIRST_WORDS: [&str; 6] = ["Angel", "Bolt", "Cat", "Dragon", "Elf", "Prof_Oak%"];
const SECOND_WORDS: [&str; 4] = ["of Embers", "the Wise", "Rider", "Æther"];
const NAME_FRAGMENTS: [&str; 10] = [
    "angel",
    "bolt",
    "dragon of",
    "e",
    "_",
    "%",
    "the wise",
    "cat 1",
    "aether",
    "OAK",
];
const VOCABULARY: [&str; 8] = [
    "alpha", "bravo", "charlie", "delta", "echo", "foxtrot", "golf", "hotel",
];
const MANA_VALUES: [f64; 9] = [0.0, 0.5, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 8.0];
const LEGALITIES: [Legality; 4] = [
    Legality::Legal,
    Legality::NotLegal,
    Legality::Banned,
    Legality::Restricted,
];
const FORMATS: [&str; 3] = ["commander", "modern", "pauper"];

#[derive(Debug, Clone)]
struct Spec {
    mana_value: f64,
    colors: u8,
    extra_identity: u8,
    type_line: &'static str,
    first: &'static str,
    second: &'static str,
    words: Vec<&'static str>,
    commander: Legality,
    modern: Option<Legality>,
    token: bool,
}

#[derive(Debug, Clone)]
struct Criteria {
    name: Option<&'static str>,
    oracle_words: Vec<&'static str>,
    type_terms: Vec<&'static str>,
    colors: Option<(u8, u8)>,
    identity: Option<u8>,
    min: Option<f64>,
    max: Option<f64>,
    legal_in: Option<&'static str>,
    include_non_deck: bool,
    by_mana_value: bool,
}

fn spec() -> impl Strategy<Value = Spec> {
    (
        prop::sample::select(MANA_VALUES.to_vec()),
        0u8..32,
        0u8..32,
        prop::sample::select(TYPE_LINES.to_vec()),
        prop::sample::select(FIRST_WORDS.to_vec()),
        prop::sample::select(SECOND_WORDS.to_vec()),
        prop::collection::vec(prop::sample::select(VOCABULARY.to_vec()), 0..5),
        prop::sample::select(LEGALITIES.to_vec()),
        prop::option::of(prop::sample::select(LEGALITIES.to_vec())),
        prop::bool::weighted(0.1),
    )
        .prop_map(
            |(
                mana_value,
                colors,
                extra_identity,
                type_line,
                first,
                second,
                words,
                commander,
                modern,
                token,
            )| Spec {
                mana_value,
                colors,
                extra_identity,
                type_line,
                first,
                second,
                words,
                commander,
                modern,
                token,
            },
        )
}

fn criteria() -> impl Strategy<Value = Criteria> {
    (
        prop::option::weighted(0.3, prop::sample::select(NAME_FRAGMENTS.to_vec())),
        prop::collection::vec(prop::sample::select(VOCABULARY.to_vec()), 0..3),
        prop::collection::vec(prop::sample::select(TYPE_TERMS.to_vec()), 0..3),
        prop::option::weighted(0.4, (0u8..32, 0u8..4)),
        prop::option::weighted(0.4, 0u8..32),
        prop::option::weighted(0.3, prop::sample::select(MANA_VALUES.to_vec())),
        prop::option::weighted(0.3, prop::sample::select(MANA_VALUES.to_vec())),
        prop::option::weighted(0.3, prop::sample::select(FORMATS.to_vec())),
        prop::bool::weighted(0.2),
        prop::bool::ANY,
    )
        .prop_map(
            |(
                name,
                oracle_words,
                type_terms,
                colors,
                identity,
                min,
                max,
                legal_in,
                include_non_deck,
                by_mana_value,
            )| Criteria {
                name,
                oracle_words,
                type_terms,
                colors,
                identity,
                min,
                max,
                legal_in,
                include_non_deck,
                by_mana_value,
            },
        )
}

fn set_of(mask: u8) -> ColorSet {
    Color::ALL
        .iter()
        .enumerate()
        .filter(|(bit, _)| mask >> bit & 1 == 1)
        .map(|(_, &color)| color)
        .collect()
}

fn card_from(base: &CardRecord, n: usize, spec: &Spec) -> CardRecord {
    let name = format!("{} {} {n}", spec.first, spec.second);
    let mut card = another_card(base, n as u128 + 1, &name);
    card.mana_value = spec.mana_value;
    card.colors = set_of(spec.colors);
    card.color_identity = set_of(spec.colors | spec.extra_identity);
    spec.type_line.clone_into(&mut card.type_line);
    card.front.oracle_text = spec.words.join(" ");
    card.layout = if spec.token {
        Layout::Token
    } else {
        Layout::Normal
    };
    card.legalities.clear();
    card.legalities.insert("commander".into(), spec.commander);
    if let Some(modern) = spec.modern {
        card.legalities.insert("modern".into(), modern);
    }
    card
}

fn color_filter(colors: u8, mode: u8) -> ColorFilter {
    ColorFilter {
        colors: set_of(colors),
        mode: match mode {
            0 => ColorMatch::Exactly,
            1 => ColorMatch::Includes,
            2 => ColorMatch::Within,
            _ => ColorMatch::Overlaps,
        },
    }
}

fn search_for(criteria: &Criteria) -> CardSearch {
    CardSearch {
        name: criteria.name.map(str::to_owned),
        oracle_text: (!criteria.oracle_words.is_empty()).then(|| criteria.oracle_words.join(" ")),
        type_terms: criteria.type_terms.iter().map(|&t| t.to_owned()).collect(),
        colors: criteria
            .colors
            .map(|(colors, mode)| color_filter(colors, mode)),
        color_identity_within: criteria.identity.map(set_of),
        min_mana_value: criteria.min,
        max_mana_value: criteria.max,
        legal_in: criteria.legal_in.map(str::to_owned),
        include_non_deck: criteria.include_non_deck,
        sort: if criteria.by_mana_value {
            SortKey::ManaValue
        } else {
            SortKey::Name
        },
    }
}

/// The plain-Rust reading of the same criteria; independent of any SQL.
fn matches(card: &CardRecord, criteria: &Criteria) -> bool {
    let words: BTreeSet<&str> = card.front.oracle_text.split_whitespace().collect();
    let color_ok = criteria.colors.is_none_or(|(mask, mode)| {
        let (have, asked) = (card.colors, set_of(mask));
        let overlaps = Color::ALL
            .iter()
            .any(|&c| have.contains(c) && asked.contains(c));
        match mode {
            0 => have == asked,
            1 => asked.is_subset_of(have),
            2 => have.is_subset_of(asked),
            _ => overlaps,
        }
    });
    (criteria.include_non_deck || card.layout.is_deck_card())
        && criteria
            .name
            .is_none_or(|fragment| normalize_name(&card.name).contains(&normalize_name(fragment)))
        && criteria.oracle_words.iter().all(|w| words.contains(w))
        && criteria
            .type_terms
            .iter()
            .all(|term| card.type_line.to_lowercase().contains(&term.to_lowercase()))
        && color_ok
        && criteria
            .identity
            .is_none_or(|mask| card.color_identity.is_subset_of(set_of(mask)))
        && criteria.min.is_none_or(|min| card.mana_value >= min)
        && criteria.max.is_none_or(|max| card.mana_value <= max)
        && criteria.legal_in.is_none_or(|format| {
            matches!(
                card.legality(format),
                Legality::Legal | Legality::Restricted
            )
        })
}

fn expected_order(cards: &[CardRecord], criteria: &Criteria) -> Vec<OracleId> {
    let mut found: Vec<&CardRecord> = cards.iter().filter(|c| matches(c, criteria)).collect();
    found.sort_by(|a, b| {
        let by_value = if criteria.by_mana_value {
            a.mana_value.total_cmp(&b.mana_value)
        } else {
            std::cmp::Ordering::Equal
        };
        by_value
            .then_with(|| normalize_name(&a.name).cmp(&normalize_name(&b.name)))
            .then_with(|| a.oracle_id.cmp(&b.oracle_id))
    });
    found.into_iter().map(|c| c.oracle_id.clone()).collect()
}

fn generated<T: std::fmt::Debug>(runner: &mut TestRunner, strategy: impl Strategy<Value = T>) -> T {
    strategy
        .new_tree(runner)
        .unwrap_or_else(|e| panic!("strategy failed: {e}"))
        .current()
}

#[sqlx::test]
async fn database_search_matches_an_in_memory_filter(pool: PgPool) {
    let mut runner = TestRunner::deterministic();
    let base = record("Shock");
    let specs = generated(&mut runner, prop::collection::vec(spec(), CARD_COUNT));
    let cards: Vec<CardRecord> = specs
        .iter()
        .enumerate()
        .map(|(n, s)| card_from(&base, n, s))
        .collect();
    db(pool.clone())
        .importer()
        .import_cards(cards.iter().cloned().map(Ok))
        .await
        .unwrap();
    let repo = repo(&pool);

    let mut non_trivial = 0;
    for _ in 0..SEARCH_COUNT {
        let criteria = generated(&mut runner, criteria());
        let expected = expected_order(&cards, &criteria);
        let search = search_for(&criteria);
        let full = repo
            .search(&search, Page::new(MAX_PAGE_SIZE, 0))
            .await
            .unwrap();
        let actual: Vec<OracleId> = full.cards.iter().map(|c| c.oracle_id.clone()).collect();
        let head = &expected[..expected.len().min(MAX_PAGE_SIZE as usize)];
        assert!(actual == head, "first page differs for {criteria:?}");
        assert_eq!(full.total, expected.len() as u64, "{criteria:?}");
        if !expected.is_empty() && expected.len() < cards.len() {
            non_trivial += 1;
        }

        // Paging through the same search yields the same sequence, with no repeats or gaps.
        for limit in [1u32, 7, 60] {
            let mut paged = Vec::new();
            let mut page = Page::new(limit, 0);
            loop {
                let result = repo.search(&search, page).await.unwrap();
                assert_eq!(
                    result.total,
                    expected.len() as u64,
                    "{criteria:?} limit {limit}"
                );
                assert!(result.cards.len() <= limit as usize);
                if result.cards.is_empty() {
                    break;
                }
                paged.extend(result.cards.iter().map(|c| c.oracle_id.clone()));
                page = page.next();
            }
            let distinct: BTreeSet<_> = paged.iter().collect();
            assert_eq!(
                distinct.len(),
                paged.len(),
                "duplicates with limit {limit}: {criteria:?}"
            );
            assert!(
                paged == expected,
                "paging with limit {limit} differs for {criteria:?}"
            );
        }
    }
    assert!(
        non_trivial > SEARCH_COUNT / 4,
        "only {non_trivial} searches were selective"
    );
}
