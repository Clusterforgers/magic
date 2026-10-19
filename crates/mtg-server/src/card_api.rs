//! The public card endpoints: `GET /api/cards` searches the card database and `GET /api/cards/{oracle_id}`
//! shows one card. Both are read-only and need no account; a server without a database answers 503
//! `cards_unavailable`.

use std::collections::HashMap;

use axum::Json;
use axum::extract::rejection::{PathRejection, QueryRejection};
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use mtg_cards::support::Reason;
use mtg_core::color::{Color, ColorSet};
use mtg_db::codec::{legality_keyword, rarity_keyword, ruling_source_keyword};
use mtg_db::{Card, CardSearch, ColorFilter, ColorMatch, Page, PrintingImages, SortKey};
use mtg_import::ids::OracleId;
use mtg_import::scryfall::ImageUris;
use mtg_protocol::rest::{
    CardColor, CardDetail, CardFaceView, CardHit, CardImages, CardSearchPage, DEFAULT_PER_PAGE,
    ImageLinks, MAX_PAGE, MAX_PER_PAGE, PrintingView, RulingView,
};
use sqlx::types::uuid::Uuid;

use crate::api::ApiError;
use crate::cards::{CardSource, CatalogError};
use crate::server::AppState;

/// Longest free text of a search, in characters.
const MAX_TEXT_CHARS: usize = 200;
/// Most words in the type filter.
const MAX_TYPE_TERMS: usize = 8;
/// Longest word of the type filter, in characters.
const MAX_TYPE_TERM_CHARS: usize = 40;
/// Longest format key.
const MAX_FORMAT_CHARS: usize = 32;
/// Highest mana value a filter may name (Gleemax costs a million).
const MAX_MANA_VALUE: f64 = 1_000_000.0;

/// The parameters `GET /api/cards` knows.
const PARAMETERS: [&str; 9] = [
    "q",
    "colors",
    "colors_match",
    "identity",
    "type",
    "cmc",
    "format",
    "page",
    "per_page",
];

fn invalid(message: impl Into<String>) -> ApiError {
    ApiError::invalid(message)
}

/// What a request asks for, in the terms of the repository.
#[derive(Debug)]
struct Search {
    criteria: CardSearch,
    page: u32,
    per_page: u32,
}

/// The colors named by a `colors` or `identity` value: letters from WUBRG in any case, or `C` alone
/// for colorless.
fn colors_of(name: &str, value: &str) -> Result<ColorSet, ApiError> {
    let rule = || {
        invalid(format!(
            "{name} is made of the letters W, U, B, R and G, or C alone for colorless"
        ))
    };
    let letters: Vec<char> = value.chars().map(|c| c.to_ascii_uppercase()).collect();
    if letters == ['C'] {
        return Ok(ColorSet::EMPTY);
    }
    letters
        .into_iter()
        .map(|letter| Color::from_symbol(letter).ok_or_else(rule))
        .collect()
}

/// A mana value filter: `3`, `2-4` or `5+`, as `(smallest, largest)`.
fn mana_values(value: &str) -> Result<(Option<f64>, Option<f64>), ApiError> {
    let rule =
        || invalid("cmc is a number such as 3, a range such as 2-4, or a minimum such as 5+");
    let number = |text: &str| {
        text.parse::<f64>()
            .ok()
            .filter(|n| n.is_finite() && (0.0..=MAX_MANA_VALUE).contains(n))
            .ok_or_else(rule)
    };
    if let Some(minimum) = value.strip_suffix('+') {
        return Ok((Some(number(minimum)?), None));
    }
    if let Some((low, high)) = value.split_once('-') {
        return Ok((Some(number(low)?), Some(number(high)?)));
    }
    let exact = number(value)?;
    Ok((Some(exact), Some(exact)))
}

fn match_mode(value: &str) -> Result<ColorMatch, ApiError> {
    match value {
        "exactly" => Ok(ColorMatch::Exactly),
        "includes" => Ok(ColorMatch::Includes),
        "within" => Ok(ColorMatch::Within),
        "overlaps" => Ok(ColorMatch::Overlaps),
        _ => Err(invalid(
            "colors_match is exactly, includes, within or overlaps",
        )),
    }
}

fn count(name: &str, value: Option<&String>, default: u32, most: u32) -> Result<u32, ApiError> {
    let Some(text) = value else {
        return Ok(default);
    };
    match text.parse::<u32>() {
        Ok(n) if (1..=most).contains(&n) => Ok(n),
        _ => Err(invalid(format!(
            "{name} is a whole number from 1 to {most}"
        ))),
    }
}

/// Reads the query string of `GET /api/cards`; every problem is a 400 that says what is wrong.
fn parse(params: &HashMap<String, String>) -> Result<Search, ApiError> {
    if let Some(unknown) = params
        .keys()
        .filter(|key| !PARAMETERS.contains(&key.as_str()))
        .min()
    {
        return Err(invalid(format!("unknown parameter {unknown:?}")));
    }
    let get = |key: &str| params.get(key).map(|v| v.trim()).filter(|v| !v.is_empty());
    let mut criteria = CardSearch::default();

    if let Some(text) = get("q") {
        if text.chars().count() > MAX_TEXT_CHARS {
            return Err(invalid(format!("q is at most {MAX_TEXT_CHARS} characters")));
        }
        criteria.text = Some(text.to_owned());
        criteria.sort = SortKey::Relevance;
    }
    if let Some(value) = get("colors") {
        let colors = colors_of("colors", value)?;
        let mode = if colors.is_empty() {
            ColorMatch::Exactly
        } else {
            get("colors_match").map_or(Ok(ColorMatch::Includes), match_mode)?
        };
        criteria.colors = Some(ColorFilter { colors, mode });
    }
    if let Some(value) = get("identity") {
        criteria.color_identity_within = Some(colors_of("identity", value)?);
    }
    if let Some(value) = get("type") {
        let terms: Vec<String> = value.split_whitespace().map(str::to_owned).collect();
        if terms.len() > MAX_TYPE_TERMS
            || terms
                .iter()
                .any(|t| t.chars().count() > MAX_TYPE_TERM_CHARS)
        {
            return Err(invalid(format!(
                "type is at most {MAX_TYPE_TERMS} words of at most {MAX_TYPE_TERM_CHARS} characters"
            )));
        }
        criteria.type_terms = terms;
    }
    if let Some(value) = get("cmc") {
        (criteria.min_mana_value, criteria.max_mana_value) = mana_values(value)?;
    }
    if let Some(value) = get("format") {
        let key = value.to_ascii_lowercase();
        let fine = key.len() <= MAX_FORMAT_CHARS
            && key
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_');
        if !fine {
            return Err(invalid(
                "format is a format key such as commander or modern",
            ));
        }
        criteria.legal_in = Some(key);
    }
    let page = count("page", params.get("page"), 1, MAX_PAGE)?;
    let per_page = match params.get("per_page") {
        // A page larger than the cap is lowered to it; nonsense is refused.
        Some(text) if text.parse::<u32>().is_ok_and(|n| n > MAX_PER_PAGE) => MAX_PER_PAGE,
        other => count("per_page", other, DEFAULT_PER_PAGE, MAX_PER_PAGE)?,
    };
    Ok(Search {
        criteria,
        page,
        per_page,
    })
}

/// The service behind the card endpoints, or the 503 of a server without a database.
pub(crate) fn source(app: &AppState) -> Result<&CardSource, ApiError> {
    app.cards().ok_or_else(|| {
        ApiError::new(
            StatusCode::SERVICE_UNAVAILABLE,
            "cards_unavailable",
            "this server runs without a database, so it has no cards",
        )
    })
}

/// A failure of the database or of the rules files, logged and answered without details.
pub(crate) fn failed(error: &impl std::fmt::Display) -> ApiError {
    tracing::error!(%error, "the card database failed");
    ApiError::new(
        StatusCode::SERVICE_UNAVAILABLE,
        "unavailable",
        "the card database is not available right now; try again later",
    )
}

impl From<CatalogError> for ApiError {
    fn from(error: CatalogError) -> ApiError {
        failed(&error)
    }
}

fn card_color(color: Color) -> CardColor {
    match color {
        Color::White => CardColor::W,
        Color::Blue => CardColor::U,
        Color::Black => CardColor::B,
        Color::Red => CardColor::R,
        Color::Green => CardColor::G,
    }
}

/// The colors of a set in WUBRG order.
pub(crate) fn card_colors(set: ColorSet) -> Vec<CardColor> {
    set.iter().map(card_color).collect()
}

fn links(images: &ImageUris) -> ImageLinks {
    ImageLinks {
        small: images.small.clone(),
        normal: images.normal.clone(),
        large: images.large.clone(),
        png: images.png.clone(),
        art_crop: images.art_crop.clone(),
        border_crop: images.border_crop.clone(),
    }
}

/// The pictures of a printing as the wire shows them.
pub(crate) fn card_images(images: &PrintingImages) -> CardImages {
    CardImages {
        card: images.card.as_ref().map(links),
        faces: images
            .faces
            .iter()
            .map(|face| face.as_ref().map(links))
            .collect(),
    }
}

/// A card as a list shows it.
pub(crate) fn hit(
    cards: &CardSource,
    card: &Card,
    images: Option<&PrintingImages>,
) -> Result<CardHit, CatalogError> {
    Ok(CardHit {
        oracle_id: card.oracle_id.to_string(),
        name: card.name.clone(),
        mana_cost: card.mana_cost.clone(),
        mana_value: card.mana_value,
        type_line: card.type_line.clone(),
        colors: card_colors(card.colors),
        color_identity: card_colors(card.color_identity),
        images: images.map(card_images).unwrap_or_default(),
        support: cards.support_of(card)?.support,
    })
}

/// `GET /api/cards`: the cards that match the query, a page at a time. Free text matches the name or
/// the rules text, names first; the other parameters narrow the result (see `docs/server.md`).
pub(crate) async fn search(
    State(app): State<AppState>,
    query: Result<Query<HashMap<String, String>>, QueryRejection>,
) -> Result<Json<CardSearchPage>, ApiError> {
    let cards = source(&app)?;
    let Query(params) = query.map_err(|_| invalid("the query string is not valid"))?;
    let Search {
        criteria,
        page,
        per_page,
    } = parse(&params)?;
    let repo = cards.database().cards();
    let offset = (page - 1).saturating_mul(per_page);
    let found = repo
        .search(&criteria, Page::new(per_page, offset))
        .await
        .map_err(|error| failed(&error))?;
    let ids: Vec<OracleId> = found.cards.iter().map(|c| c.oracle_id.clone()).collect();
    let images = repo
        .default_images(&ids)
        .await
        .map_err(|error| failed(&error))?;
    let hits = found
        .cards
        .iter()
        .map(|card| hit(cards, card, images.get(&card.oracle_id)))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Json(CardSearchPage {
        cards: hits,
        total: found.total,
        page,
        per_page,
    }))
}

/// `GET /api/cards/{oracle_id}`: one card with its faces, printings, rulings and legalities. A
/// malformed id and an unknown card look alike.
pub(crate) async fn detail(
    State(app): State<AppState>,
    path: Result<Path<String>, PathRejection>,
) -> Result<Json<CardDetail>, ApiError> {
    let cards = source(&app)?;
    let Path(id) = path.map_err(|_| ApiError::not_found())?;
    let id = Uuid::parse_str(&id)
        .map(|uuid| OracleId(uuid.to_string()))
        .map_err(|_| ApiError::not_found())?;
    let repo = cards.database().cards();
    let found = repo
        .detail(&id)
        .await
        .map_err(|error| failed(&error))?
        .ok_or_else(ApiError::not_found)?;
    let images = repo
        .default_images(std::slice::from_ref(&id))
        .await
        .map_err(|error| failed(&error))?;
    let record = cards.support_of(&found.card)?;
    let card = &found.card;
    Ok(Json(CardDetail {
        card: hit(cards, card, images.get(&id))?,
        oracle_text: card.oracle_text.clone(),
        keywords: card.keywords.clone(),
        faces: card
            .faces
            .iter()
            .map(|face| CardFaceView {
                name: face.name.clone(),
                mana_cost: face.mana_cost.clone(),
                type_line: face.type_line.clone(),
                oracle_text: face.oracle_text.clone(),
                power: face.power.clone(),
                toughness: face.toughness.clone(),
                loyalty: face.loyalty.clone(),
                defense: face.defense.clone(),
                colors: card_colors(face.colors),
            })
            .collect(),
        legalities: card
            .legalities
            .iter()
            .map(|(format, legality)| (format.clone(), legality_keyword(*legality).to_owned()))
            .collect(),
        printings: found
            .printings
            .iter()
            .map(|printing| PrintingView {
                printing_id: printing.printing_id.to_string(),
                set: printing.set.clone(),
                collector_number: printing.collector_number.clone(),
                rarity: printing.rarity.map(|r| rarity_keyword(r).to_owned()),
                released_at: printing.released_at.map(|date| date.to_string()),
                lang: printing.lang.clone(),
                digital: printing.digital,
                images: card_images(&printing.images),
            })
            .collect(),
        rulings: found
            .rulings
            .iter()
            .map(|ruling| RulingView {
                source: ruling_source_keyword(ruling.source).to_owned(),
                published_at: ruling.published_at.map(|date| date.to_string()),
                comment: ruling.comment.clone(),
            })
            .collect(),
        reasons: record.clauses.iter().map(Reason::from).collect(),
    }))
}

#[cfg(test)]
mod tests {
    //! Goal: the query string of the card search is read exactly: every parameter maps to one criterion of the repository, blank values are absent, the page size is capped, and anything else is a 400 that names the problem.
    use axum::http::StatusCode;
    use rstest::rstest;

    use super::*;

    fn params(pairs: &[(&str, &str)]) -> HashMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
            .collect()
    }

    fn ok(pairs: &[(&str, &str)]) -> Search {
        parse(&params(pairs)).unwrap_or_else(|e| panic!("{pairs:?}: {e:?}"))
    }

    fn set(letters: &str) -> ColorSet {
        letters.chars().filter_map(Color::from_symbol).collect()
    }

    #[test]
    fn no_parameters_ask_for_the_first_page_by_name() {
        let search = ok(&[]);
        assert_eq!((search.page, search.per_page), (1, DEFAULT_PER_PAGE));
        assert_eq!(search.criteria.sort, SortKey::Name);
        assert!(search.criteria.text.is_none() && search.criteria.legal_in.is_none());
    }

    #[test]
    fn every_parameter_reaches_its_criterion() {
        let search = ok(&[
            ("q", " sol ring "),
            ("colors", "wu"),
            ("colors_match", "within"),
            ("identity", "BG"),
            ("type", "legendary  creature"),
            ("cmc", "2-4"),
            ("format", "Commander"),
            ("page", "3"),
            ("per_page", "25"),
        ]);
        let c = &search.criteria;
        assert_eq!(c.text.as_deref(), Some("sol ring"));
        assert_eq!(c.sort, SortKey::Relevance);
        assert_eq!(
            c.colors.map(|f| (f.colors, f.mode)),
            Some((set("WU"), ColorMatch::Within))
        );
        assert_eq!(c.color_identity_within, Some(set("BG")));
        assert_eq!(c.type_terms, ["legendary", "creature"]);
        assert_eq!((c.min_mana_value, c.max_mana_value), (Some(2.0), Some(4.0)));
        assert_eq!(c.legal_in.as_deref(), Some("commander"));
        assert_eq!((search.page, search.per_page), (3, 25));
    }

    #[rstest]
    #[case("3", Some(3.0), Some(3.0))]
    #[case("0", Some(0.0), Some(0.0))]
    #[case("0.5", Some(0.5), Some(0.5))]
    #[case("2-4", Some(2.0), Some(4.0))]
    #[case("5+", Some(5.0), None)]
    #[case("7-3", Some(7.0), Some(3.0))]
    fn mana_values_read_as_a_value_a_range_or_a_minimum(
        #[case] value: &str,
        #[case] low: Option<f64>,
        #[case] high: Option<f64>,
    ) {
        let search = ok(&[("cmc", value)]);
        assert_eq!(
            (
                search.criteria.min_mana_value,
                search.criteria.max_mana_value
            ),
            (low, high)
        );
    }

    #[rstest]
    #[case("w", "W", ColorMatch::Includes)]
    #[case("WUBRG", "WUBRG", ColorMatch::Includes)]
    #[case("gw", "WG", ColorMatch::Includes)]
    #[case("C", "", ColorMatch::Exactly)]
    #[case("c", "", ColorMatch::Exactly)]
    fn colors_are_letters_and_c_alone_means_colorless(
        #[case] value: &str,
        #[case] expected: &str,
        #[case] mode: ColorMatch,
    ) {
        let search = ok(&[("colors", value), ("colors_match", "overlaps")]);
        let filter = search.criteria.colors.unwrap();
        assert_eq!(filter.colors, set(expected));
        // Colorless is always an exact match; the other modes would mean nothing or everything.
        if expected.is_empty() {
            assert_eq!(filter.mode, mode);
        } else {
            assert_eq!(filter.mode, ColorMatch::Overlaps);
        }
        let identity = ok(&[("identity", value)]);
        assert_eq!(identity.criteria.color_identity_within, Some(set(expected)));
    }

    #[test]
    fn blank_values_are_absent_and_the_page_size_is_capped() {
        let search = ok(&[
            ("q", "  "),
            ("colors", ""),
            ("type", " "),
            ("per_page", "100000"),
        ]);
        assert!(search.criteria.text.is_none() && search.criteria.colors.is_none());
        assert_eq!(search.criteria.type_terms.len(), 0);
        assert_eq!(search.per_page, MAX_PER_PAGE);
        assert_eq!(ok(&[("per_page", "100")]).per_page, 100);
        assert_eq!(ok(&[("page", "10000")]).page, MAX_PAGE);
    }

    #[rstest]
    #[case::unknown_parameter(&[("sort", "name")], "unknown parameter")]
    #[case::typo(&[("colours", "W")], "unknown parameter")]
    #[case::bad_color(&[("colors", "WX")], "colors is made of")]
    #[case::colorless_and_a_color(&[("identity", "CW")], "identity is made of")]
    #[case::bad_mode(&[("colors", "W"), ("colors_match", "some")], "colors_match")]
    #[case::bad_cmc(&[("cmc", "many")], "cmc is")]
    #[case::negative_cmc(&[("cmc", "-1")], "cmc is")]
    #[case::open_range(&[("cmc", "2-")], "cmc is")]
    #[case::huge_cmc(&[("cmc", "9999999")], "cmc is")]
    #[case::not_a_number(&[("cmc", "NaN")], "cmc is")]
    #[case::bad_format(&[("format", "com mander")], "format is")]
    #[case::long_format(&[("format", "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa")], "format is")]
    #[case::page_zero(&[("page", "0")], "page is")]
    #[case::deep_page(&[("page", "10001")], "page is")]
    #[case::page_text(&[("page", "one")], "page is")]
    #[case::negative_page(&[("page", "-1")], "page is")]
    #[case::per_page_zero(&[("per_page", "0")], "per_page is")]
    #[case::per_page_text(&[("per_page", "lots")], "per_page is")]
    fn nonsense_is_a_400_that_says_what_is_wrong(
        #[case] pairs: &[(&str, &str)],
        #[case] fragment: &str,
    ) {
        let error = parse(&params(pairs)).unwrap_err();
        assert_eq!(error.status(), StatusCode::BAD_REQUEST);
        assert_eq!(error.code(), "invalid_request");
        assert!(error.message().contains(fragment), "{}", error.message());
    }

    #[test]
    fn long_texts_and_many_type_words_are_refused() {
        let long = "x".repeat(MAX_TEXT_CHARS + 1);
        assert!(parse(&params(&[("q", &long)])).is_err());
        assert!(parse(&params(&[("q", &long[..MAX_TEXT_CHARS])])).is_ok());
        let many = (0..=MAX_TYPE_TERMS)
            .map(|n| format!("t{n}"))
            .collect::<Vec<_>>()
            .join(" ");
        assert!(parse(&params(&[("type", &many)])).is_err());
        let wide = "y".repeat(MAX_TYPE_TERM_CHARS + 1);
        assert!(parse(&params(&[("type", &wide)])).is_err());
    }
}
