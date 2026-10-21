//! Goal: shared case tables and runner for the table-driven card search tests.

use mtg_db::cards::MAX_PAGE_SIZE;
use mtg_db::{CardRepo, CardSearch, ColorFilter, ColorMatch, Page};

use super::{colors, in_search_order, names};

/// One search and the cards it must return, in order.
pub struct Case {
    pub label: &'static str,
    pub search: CardSearch,
    pub expected: Vec<String>,
}

/// A case whose results are the named cards in default (name) order.
pub fn found(label: &'static str, search: CardSearch, names: &[&str]) -> Case {
    Case {
        label,
        search,
        expected: in_search_order(names),
    }
}

/// A case whose results must come back in exactly the given order.
pub fn ordered(label: &'static str, search: CardSearch, names: &[&str]) -> Case {
    Case {
        label,
        search,
        expected: names.iter().map(|&n| n.to_owned()).collect(),
    }
}

pub fn with(f: impl FnOnce(&mut CardSearch)) -> CardSearch {
    let mut search = CardSearch::default();
    f(&mut search);
    search
}

pub fn colors_are(symbols: &str, mode: ColorMatch) -> CardSearch {
    with(|s| {
        s.colors = Some(ColorFilter {
            colors: colors(symbols),
            mode,
        });
    })
}

pub fn mana(min: Option<f64>, max: Option<f64>) -> CardSearch {
    with(|s| {
        s.min_mana_value = min;
        s.max_mana_value = max;
    })
}

pub fn types(terms: &[&str]) -> CardSearch {
    with(|s| s.type_terms = terms.iter().map(|&t| t.to_owned()).collect())
}

pub fn text(query: &str) -> CardSearch {
    with(|s| s.oracle_text = Some(query.to_owned()))
}

pub fn legal(format: &str) -> CardSearch {
    with(|s| s.legal_in = Some(format.to_owned()))
}

pub fn named(fragment: &str) -> CardSearch {
    with(|s| s.name = Some(fragment.to_owned()))
}

pub const ALL_DECK_CARDS: [&str; 20] = [
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

pub const COLORLESS: [&str; 8] = [
    "Æther Vial",
    "Arcane Signet",
    "Forest",
    "Hazmat Suit (Used)",
    "Island",
    "Mountain",
    "Mystery Card",
    "Sol Ring",
];

/// Runs every case against the repository and reports all mismatches at once.
pub async fn run(repo: &CardRepo, cases: Vec<Case>) {
    let mut failures = Vec::new();
    for case in cases {
        let page = repo
            .search(&case.search, Page::new(MAX_PAGE_SIZE, 0))
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
