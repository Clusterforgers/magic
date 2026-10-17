//! Filtered, paginated card search with a total, stable ordering.

use mtg_core::color::ColorSet;
use mtg_import::resolve::normalize_name;
use serde_json::json;
use sqlx::{Postgres, QueryBuilder};

use super::CardRepo;
use crate::colors::to_mask;
use crate::error::DbError;
use crate::model::Card;
use crate::rows::{CARD_COLUMNS, CardRow};

/// Page size used by [`Page::default`].
pub const DEFAULT_PAGE_SIZE: u32 = 50;

/// Largest page a search returns.
pub const MAX_PAGE_SIZE: u32 = 200;

/// A window into an ordered result set.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Page {
    limit: u32,
    offset: u32,
}

impl Page {
    /// A page of `limit` rows (clamped to `1..=MAX_PAGE_SIZE`) after skipping `offset` rows.
    pub fn new(limit: u32, offset: u32) -> Self {
        Self {
            limit: limit.clamp(1, MAX_PAGE_SIZE),
            offset,
        }
    }

    /// Number of rows per page.
    pub fn limit(self) -> u32 {
        self.limit
    }

    /// Number of rows skipped before the page starts.
    pub fn offset(self) -> u32 {
        self.offset
    }

    /// The page that follows this one.
    #[must_use]
    pub fn next(self) -> Self {
        Self {
            limit: self.limit,
            offset: self.offset.saturating_add(self.limit),
        }
    }
}

impl Default for Page {
    fn default() -> Self {
        Self::new(DEFAULT_PAGE_SIZE, 0)
    }
}

/// How a color filter compares the given colors with a card's colors.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorMatch {
    /// The card has exactly these colors (an empty set means colorless).
    Exactly,
    /// The card has at least these colors.
    Includes,
    /// The card has no colors beyond these (an empty set means colorless).
    Within,
    /// The card has at least one of these colors.
    Overlaps,
}

/// Restricts search results by card color.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ColorFilter {
    /// Colors to compare against.
    pub colors: ColorSet,
    /// How to compare.
    pub mode: ColorMatch,
}

/// Primary sort key of a search; ties always fall back to name and oracle id.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum SortKey {
    /// Alphabetical by normalized name.
    #[default]
    Name,
    /// Ascending mana value.
    ManaValue,
}

/// Criteria of a card search; every set criterion must match.
#[derive(Debug, Clone, Default)]
pub struct CardSearch {
    /// Text contained in the name, ignoring case and accents.
    pub name: Option<String>,
    /// Full-text query over rules text, in web-search syntax (`"exact phrase"`, `-excluded`, `or`).
    pub oracle_text: Option<String>,
    /// Terms that must each appear in the type line, ignoring case.
    pub type_terms: Vec<String>,
    /// Color restriction.
    pub colors: Option<ColorFilter>,
    /// Commander deckbuilding: the card's color identity must be a subset of these colors.
    pub color_identity_within: Option<ColorSet>,
    /// Smallest mana value, inclusive.
    pub min_mana_value: Option<f64>,
    /// Largest mana value, inclusive.
    pub max_mana_value: Option<f64>,
    /// Format (lowercase Scryfall name) the card must be legal or restricted in.
    pub legal_in: Option<String>,
    /// Also return tokens, emblems and other cards that cannot be put in a deck.
    pub include_non_deck: bool,
    /// Primary sort key.
    pub sort: SortKey,
}

/// One page of search results.
#[derive(Debug, Clone, PartialEq)]
pub struct CardPage {
    /// Cards on this page, in search order.
    pub cards: Vec<Card>,
    /// Number of cards matching the criteria across all pages.
    pub total: u64,
    /// The window these cards were taken from.
    pub page: Page,
}

/// The trimmed text of a filter, or `None` when it is absent or blank.
fn present(value: Option<&String>) -> Option<&str> {
    value.map(|text| text.trim()).filter(|text| !text.is_empty())
}

/// A `LIKE` pattern matching any text that contains `term` literally.
fn like_contains(term: &str) -> String {
    let mut pattern = String::with_capacity(term.len() + 2);
    pattern.push('%');
    for c in term.chars() {
        if matches!(c, '\\' | '%' | '_') {
            pattern.push('\\');
        }
        pattern.push(c);
    }
    pattern.push('%');
    pattern
}

impl CardSearch {
    fn push_filters(&self, query: &mut QueryBuilder<Postgres>) {
        query.push(" WHERE true");
        if !self.include_non_deck {
            query.push(" AND is_deck_card");
        }
        if let Some(name) = present(self.name.as_ref()) {
            let key = normalize_name(name);
            query.push(" AND name_normalized LIKE ").push_bind(like_contains(&key));
        }
        if let Some(text) = present(self.oracle_text.as_ref()) {
            query
                .push(" AND oracle_tsv @@ websearch_to_tsquery('english', ")
                .push_bind(text.to_owned())
                .push(")");
        }
        for term in self.type_terms.iter().filter_map(|t| present(Some(t))) {
            query.push(" AND type_line ILIKE ").push_bind(like_contains(term));
        }
        if let Some(filter) = self.colors {
            let mask = to_mask(filter.colors);
            match filter.mode {
                ColorMatch::Exactly => query.push(" AND colors = ").push_bind(mask),
                ColorMatch::Includes => query
                    .push(" AND (colors & ")
                    .push_bind(mask)
                    .push(") = ")
                    .push_bind(mask),
                ColorMatch::Within => query.push(" AND (colors & ~").push_bind(mask).push(") = 0"),
                ColorMatch::Overlaps => query
                    .push(" AND (colors & ")
                    .push_bind(mask)
                    .push(") <> 0"),
            };
        }
        if let Some(allowed) = self.color_identity_within {
            query
                .push(" AND (color_identity & ~")
                .push_bind(to_mask(allowed))
                .push(") = 0");
        }
        if let Some(min) = self.min_mana_value.filter(|v| !v.is_nan()) {
            query.push(" AND mana_value >= ").push_bind(min);
        }
        if let Some(max) = self.max_mana_value.filter(|v| !v.is_nan()) {
            query.push(" AND mana_value <= ").push_bind(max);
        }
        if let Some(format) = present(self.legal_in.as_ref()) {
            let format = format.to_lowercase();
            // Containment (rather than ->>) lets the GIN index on `legalities` serve this.
            query
                .push(" AND (legalities @> ")
                .push_bind(json!({ &format: "legal" }))
                .push(" OR legalities @> ")
                .push_bind(json!({ &format: "restricted" }))
                .push(")");
        }
    }

    fn push_order(&self, query: &mut QueryBuilder<Postgres>) {
        query.push(match self.sort {
            SortKey::Name => " ORDER BY name_normalized, oracle_id",
            SortKey::ManaValue => " ORDER BY mana_value, name_normalized, oracle_id",
        });
    }
}

impl CardRepo {
    /// Cards matching `criteria`, one `page` at a time, in a total order (no ties, so pages never overlap or skip).
    pub async fn search(&self, criteria: &CardSearch, page: Page) -> Result<CardPage, DbError> {
        let mut count = QueryBuilder::<Postgres>::new("SELECT count(*) FROM cards");
        criteria.push_filters(&mut count);
        let total: i64 = count
            .build_query_scalar()
            .fetch_one(&self.pool)
            .await?;

        let mut select = QueryBuilder::<Postgres>::new(format!("SELECT {CARD_COLUMNS} FROM cards"));
        criteria.push_filters(&mut select);
        criteria.push_order(&mut select);
        select.push(" LIMIT ").push_bind(i64::from(page.limit));
        select.push(" OFFSET ").push_bind(i64::from(page.offset));
        let rows: Vec<CardRow> = select.build_query_as().fetch_all(&self.pool).await?;

        Ok(CardPage {
            cards: rows
                .into_iter()
                .map(Card::try_from)
                .collect::<Result<_, _>>()?,
            total: total.unsigned_abs(),
            page,
        })
    }
}

#[cfg(test)]
mod tests {
    //! Goal: user-supplied text can never change the shape of a query, pages are clamped to safe sizes, and blank filters are ignored.
    use rstest::rstest;

    use super::*;

    #[rstest]
    #[case("bolt", "%bolt%")]
    #[case("100%", "%100\\%%")]
    #[case("a_b", "%a\\_b%")]
    #[case("back\\slash", "%back\\\\slash%")]
    #[case("", "%%")]
    fn like_patterns_escape_wildcards(#[case] term: &str, #[case] pattern: &str) {
        assert_eq!(like_contains(term), pattern);
    }

    #[rstest]
    #[case(0, 1)]
    #[case(1, 1)]
    #[case(50, 50)]
    #[case(MAX_PAGE_SIZE, MAX_PAGE_SIZE)]
    #[case(MAX_PAGE_SIZE + 1, MAX_PAGE_SIZE)]
    #[case(u32::MAX, MAX_PAGE_SIZE)]
    fn page_sizes_are_clamped(#[case] requested: u32, #[case] actual: u32) {
        assert_eq!(Page::new(requested, 7).limit(), actual);
        assert_eq!(Page::new(requested, 7).offset(), 7);
    }

    #[test]
    fn next_page_advances_by_the_limit_without_overflowing() {
        assert_eq!(Page::new(20, 40).next(), Page::new(20, 60));
        assert_eq!(Page::new(20, u32::MAX - 5).next().offset(), u32::MAX);
    }

    #[rstest]
    #[case(None, None)]
    #[case(Some(""), None)]
    #[case(Some("  \t"), None)]
    #[case(Some(" bolt "), Some("bolt"))]
    fn blank_filters_are_absent(#[case] input: Option<&str>, #[case] expected: Option<&str>) {
        let owned = input.map(str::to_owned);
        assert_eq!(present(owned.as_ref()), expected);
    }

    #[test]
    fn criteria_without_filters_only_exclude_non_deck_cards() {
        let mut query = QueryBuilder::<Postgres>::new("SELECT 1");
        CardSearch::default().push_filters(&mut query);
        assert_eq!(query.sql().as_str(), "SELECT 1 WHERE true AND is_deck_card");
    }

    #[test]
    fn user_text_is_bound_never_spliced_into_sql() {
        let criteria = CardSearch {
            name: Some("'; DROP TABLE cards; --".into()),
            oracle_text: Some("x') OR ('1'='1".into()),
            type_terms: vec!["\" OR 1=1 --".into()],
            legal_in: Some("commander'; --".into()),
            ..CardSearch::default()
        };
        let mut query = QueryBuilder::<Postgres>::new("SELECT 1");
        criteria.push_filters(&mut query);
        let sql = query.sql();
        assert!(!sql.as_str().contains("DROP") && !sql.as_str().contains("1=1"), "{}", sql.as_str());
    }
}
