//! What the deck endpoints know of the cards in a deck: the rules a request must follow, the rows of
//! the card database with their art, and the check run over them.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use axum::http::StatusCode;
use mtg_cards::deck::{
    CardFacts, DeckLine as CheckLine, DeckReport, DeckZone, SUPPORTED_FORMATS, deck_report,
};
use mtg_db::{Card, DeckLine as StoredLine, DeckZone as StoredZone, Printing, PrintingImages};
use mtg_import::ids::{OracleId, PrintingId};
use mtg_protocol::rest::{
    DeckBody, DeckLineBody, DeckLineView, MAX_DECK_CARDS, MAX_DECK_LINES, MAX_DECK_NAME_CHARS,
    MAX_DECK_NOTES_CHARS,
};
use sqlx::types::uuid::Uuid;

use crate::api::{ApiError, is_unsafe_in_names};
use crate::card_api::{failed, hit};
use crate::cards::{CardSource, CatalogError, card_facts};

/// Most copies of a card on one line.
const MAX_LINE_COUNT: u32 = 999;
/// Most card names an error lists.
const NAMED_AT_MOST: usize = 5;

/// One line of a deck in the form the endpoints work with: ids in their canonical spelling.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Line {
    pub(crate) zone: DeckZone,
    pub(crate) oracle_id: String,
    pub(crate) printing_id: Option<String>,
    pub(crate) count: u32,
}

fn stored_zone(zone: DeckZone) -> StoredZone {
    match zone {
        DeckZone::Main => StoredZone::Main,
        DeckZone::Sideboard => StoredZone::Sideboard,
        DeckZone::Commander => StoredZone::Commander,
        DeckZone::Companion => StoredZone::Companion,
    }
}

fn zone_of(zone: StoredZone) -> DeckZone {
    match zone {
        StoredZone::Main => DeckZone::Main,
        StoredZone::Sideboard => DeckZone::Sideboard,
        StoredZone::Commander => DeckZone::Commander,
        StoredZone::Companion => DeckZone::Companion,
    }
}

impl Line {
    pub(crate) fn from_stored(line: &StoredLine) -> Line {
        Line {
            zone: zone_of(line.zone),
            oracle_id: line.oracle_id.to_string(),
            printing_id: line.printing_id.as_ref().map(ToString::to_string),
            count: line.count,
        }
    }

    pub(crate) fn to_stored(&self) -> StoredLine {
        StoredLine {
            zone: stored_zone(self.zone),
            oracle_id: OracleId(self.oracle_id.clone()),
            printing_id: self.printing_id.clone().map(PrintingId),
            count: self.count,
        }
    }

    pub(crate) fn to_check(&self) -> CheckLine {
        CheckLine {
            zone: self.zone,
            oracle_id: self.oracle_id.clone(),
            count: self.count,
        }
    }
}

pub(crate) fn invalid_deck(message: impl Into<String>) -> ApiError {
    ApiError::new(StatusCode::BAD_REQUEST, "invalid_deck", message)
}

/// A deck as a request describes it, checked against the rules that need no database.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Checked {
    pub(crate) name: String,
    pub(crate) format: String,
    pub(crate) notes: String,
    pub(crate) lines: Vec<Line>,
}

/// The format key of a request, in lower case, if the deck check has rules for it.
pub(crate) fn check_format(format: &str) -> Result<String, ApiError> {
    let key = format.trim().to_ascii_lowercase();
    if SUPPORTED_FORMATS.contains(&key.as_str()) {
        Ok(key)
    } else {
        Err(invalid_deck(format!(
            "the format must be one of {}",
            SUPPORTED_FORMATS.join(", ")
        )))
    }
}

/// An oracle or printing id in the one spelling the database answers with.
fn canonical(id: &str) -> Option<String> {
    Uuid::parse_str(id.trim()).ok().map(|uuid| uuid.to_string())
}

/// The lines of a request: at most [`MAX_DECK_LINES`] lines and [`MAX_DECK_CARDS`] copies, each
/// line with a count of 1 to 999 and ids that are ids.
pub(crate) fn check_lines(cards: &[DeckLineBody]) -> Result<Vec<Line>, ApiError> {
    if cards.len() > MAX_DECK_LINES {
        return Err(invalid_deck(format!(
            "a deck has at most {MAX_DECK_LINES} lines"
        )));
    }
    let mut copies = 0u32;
    let mut lines = Vec::with_capacity(cards.len());
    for (index, card) in cards.iter().enumerate() {
        let at = index + 1;
        if !(1..=MAX_LINE_COUNT).contains(&card.count) {
            return Err(invalid_deck(format!(
                "line {at}: the count is 1 to {MAX_LINE_COUNT}"
            )));
        }
        copies = copies.saturating_add(card.count);
        let oracle_id = canonical(&card.oracle_id)
            .ok_or_else(|| invalid_deck(format!("line {at}: oracle_id is not an oracle id")))?;
        let printing_id = card
            .printing_id
            .as_deref()
            .map(|id| {
                canonical(id).ok_or_else(|| {
                    invalid_deck(format!("line {at}: printing_id is not a printing id"))
                })
            })
            .transpose()?;
        lines.push(Line {
            zone: card.zone,
            oracle_id,
            printing_id,
            count: card.count,
        });
    }
    if copies > MAX_DECK_CARDS {
        return Err(invalid_deck(format!(
            "a deck has at most {MAX_DECK_CARDS} cards"
        )));
    }
    Ok(lines)
}

/// Checks a deck body against the rules that need no database: the name, the notes, the format and the lines.
pub(crate) fn check_body(body: &DeckBody) -> Result<Checked, ApiError> {
    let name = body.name.trim();
    let length = name.chars().count();
    if length == 0 || length > MAX_DECK_NAME_CHARS || name.chars().any(is_unsafe_in_names) {
        return Err(invalid_deck(format!(
            "the name must be 1 to {MAX_DECK_NAME_CHARS} characters without control characters"
        )));
    }
    if body.notes.chars().count() > MAX_DECK_NOTES_CHARS {
        return Err(invalid_deck(format!(
            "the notes are at most {MAX_DECK_NOTES_CHARS} characters"
        )));
    }
    if body
        .notes
        .chars()
        .any(|c| c.is_control() && !matches!(c, '\n' | '\r' | '\t'))
    {
        return Err(invalid_deck(
            "the notes may hold line breaks and tabs, no other control characters",
        ));
    }
    Ok(Checked {
        name: name.to_owned(),
        format: check_format(&body.format)?,
        notes: body.notes.clone(),
        lines: check_lines(&body.cards)?,
    })
}

/// The cards behind a set of deck lines: their rows, the facts the check reads, and the art.
pub(crate) struct Loaded {
    cards: BTreeMap<String, Card>,
    facts: BTreeMap<String, CardFacts>,
    defaults: HashMap<OracleId, PrintingImages>,
    printings: HashMap<String, Printing>,
}

impl Loaded {
    /// Reads every distinct card of `lines` and the printings they name, in three queries.
    pub(crate) async fn load(source: &CardSource, lines: &[Line]) -> Result<Loaded, ApiError> {
        let repo = source.database().cards();
        let ids: Vec<OracleId> = lines
            .iter()
            .map(|line| line.oracle_id.clone())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .map(OracleId)
            .collect();
        let found = repo.get_many(&ids).await.map_err(|error| failed(&error))?;
        let defaults = repo
            .default_images(&ids)
            .await
            .map_err(|error| failed(&error))?;
        let named: Vec<PrintingId> = lines
            .iter()
            .filter_map(|line| line.printing_id.clone())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .map(PrintingId)
            .collect();
        let printings = if named.is_empty() {
            Vec::new()
        } else {
            repo.printings(&named)
                .await
                .map_err(|error| failed(&error))?
        };
        let facts = found
            .iter()
            .map(|card| (card.oracle_id.to_string(), card_facts(card)))
            .collect();
        Ok(Loaded {
            cards: found
                .into_iter()
                .map(|card| (card.oracle_id.to_string(), card))
                .collect(),
            facts,
            defaults,
            printings: printings
                .into_iter()
                .map(|printing| (printing.printing_id.to_string(), printing))
                .collect(),
        })
    }

    /// The stored card behind an oracle id.
    pub(crate) fn card(&self, oracle_id: &str) -> Option<&Card> {
        self.cards.get(oracle_id)
    }

    /// The stored printing behind a printing id.
    pub(crate) fn printing(&self, printing_id: &str) -> Option<&Printing> {
        self.printings.get(printing_id)
    }

    /// Refuses lines of cards the database does not hold as deck cards (tokens and emblems are not),
    /// and printings that do not exist or belong to another card.
    pub(crate) fn require(&self, lines: &[Line]) -> Result<(), ApiError> {
        let unknown: Vec<&str> = lines
            .iter()
            .map(|line| line.oracle_id.as_str())
            .filter(|id| self.cards.get(*id).is_none_or(|card| !card.is_deck_card))
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        if !unknown.is_empty() {
            let shown = unknown
                .iter()
                .take(NAMED_AT_MOST)
                .copied()
                .collect::<Vec<_>>()
                .join(", ");
            let more = unknown.len().saturating_sub(NAMED_AT_MOST);
            let tail = if more > 0 {
                format!(" and {more} more")
            } else {
                String::new()
            };
            return Err(ApiError::new(
                StatusCode::BAD_REQUEST,
                "unknown_cards",
                format!("no deck card for the oracle id(s) {shown}{tail}"),
            ));
        }
        for line in lines {
            let Some(id) = &line.printing_id else {
                continue;
            };
            let belongs = self
                .printings
                .get(id)
                .is_some_and(|printing| printing.oracle_id.as_str() == line.oracle_id);
            if !belongs {
                return Err(invalid_deck(format!(
                    "the printing {id} does not exist or is not a printing of the card {}",
                    line.oracle_id
                )));
            }
        }
        Ok(())
    }

    /// The deck checked: legality in `format` and the support of every card.
    pub(crate) fn report(
        &self,
        source: &CardSource,
        format: &str,
        lines: &[Line],
    ) -> Result<DeckReport, CatalogError> {
        let lines: Vec<CheckLine> = lines.iter().map(Line::to_check).collect();
        Ok(deck_report(
            format,
            &lines,
            &self.facts,
            source.library()?,
            source.caps(),
        ))
    }

    /// A line as a list shows it, with the art of the chosen printing or else of the default one;
    /// `None` for a card the database does not hold.
    pub(crate) fn view(
        &self,
        source: &CardSource,
        line: &Line,
    ) -> Result<Option<DeckLineView>, CatalogError> {
        let Some(card) = self.cards.get(&line.oracle_id) else {
            return Ok(None);
        };
        let images = line
            .printing_id
            .as_ref()
            .and_then(|id| self.printings.get(id))
            .map(|printing| &printing.images)
            .or_else(|| self.defaults.get(&card.oracle_id));
        Ok(Some(DeckLineView {
            zone: line.zone,
            count: line.count,
            printing_id: line.printing_id.clone(),
            card: hit(source, card, images)?,
        }))
    }
}

#[cfg(test)]
mod tests {
    //! Goal: a deck request is held to every rule that needs no database: the name, the notes, the format key, the number of lines and cards, each count and each id; ids come out in one spelling, and each wrong value is a 400 that names the rule.
    use axum::http::StatusCode;
    use rstest::rstest;

    use super::*;

    const A: &str = "4457ed35-7c10-48c8-9776-456485fdf070";
    const P: &str = "e3285e6b-3e79-4d7c-bf96-d920f973b122";

    fn line(zone: DeckZone, id: &str, printing: Option<&str>, count: u32) -> DeckLineBody {
        DeckLineBody {
            zone,
            oracle_id: id.to_owned(),
            printing_id: printing.map(str::to_owned),
            count,
        }
    }

    fn body(name: &str, format: &str, notes: &str, cards: Vec<DeckLineBody>) -> DeckBody {
        DeckBody {
            name: name.to_owned(),
            format: format.to_owned(),
            notes: notes.to_owned(),
            cards,
        }
    }

    fn fine() -> DeckBody {
        body("Burn", "modern", "", vec![line(DeckZone::Main, A, None, 4)])
    }

    #[test]
    fn a_fine_body_is_kept_with_ids_in_one_spelling() {
        let mut deck = fine();
        deck.name = "  Burn  ".into();
        deck.format = " Modern ".into();
        deck.notes = "two\nlines\tand a tab".into();
        deck.cards = vec![
            line(
                DeckZone::Main,
                &A.to_uppercase(),
                Some(&P.replace('-', "")),
                4,
            ),
            line(DeckZone::Sideboard, A, None, 200),
        ];
        let checked = check_body(&deck).unwrap();
        assert_eq!(checked.name, "Burn");
        assert_eq!(checked.format, "modern");
        assert_eq!(checked.notes, "two\nlines\tand a tab");
        assert_eq!(
            checked.lines,
            vec![
                Line {
                    zone: DeckZone::Main,
                    oracle_id: A.to_owned(),
                    printing_id: Some(P.to_owned()),
                    count: 4
                },
                Line {
                    zone: DeckZone::Sideboard,
                    oracle_id: A.to_owned(),
                    printing_id: None,
                    count: 200
                }
            ]
        );
    }

    #[test]
    fn a_deck_may_have_no_cards_yet() {
        let mut deck = fine();
        deck.cards.clear();
        assert_eq!(check_body(&deck).unwrap().lines, Vec::new());
    }

    #[rstest]
    #[case::empty_name("", "name")]
    #[case::blank_name("   ", "name")]
    #[case::control_in_name("a\u{7}b", "name")]
    #[case::direction_mark_in_name("a\u{202e}b", "name")]
    fn names_follow_the_rules(#[case] name: &str, #[case] fragment: &str) {
        let mut deck = fine();
        deck.name = name.to_owned();
        let error = check_body(&deck).unwrap_err();
        assert_eq!(
            (error.status(), error.code()),
            (StatusCode::BAD_REQUEST, "invalid_deck")
        );
        assert!(error.message().contains(fragment), "{}", error.message());
    }

    #[test]
    fn the_name_may_be_as_long_as_the_limit_and_no_longer() {
        let mut deck = fine();
        deck.name = "n".repeat(MAX_DECK_NAME_CHARS);
        assert!(check_body(&deck).is_ok());
        deck.name = "n".repeat(MAX_DECK_NAME_CHARS + 1);
        assert!(check_body(&deck).is_err());
        deck.name = "\u{e9}".repeat(MAX_DECK_NAME_CHARS);
        assert!(
            check_body(&deck).is_ok(),
            "characters are counted, not bytes"
        );
    }

    #[test]
    fn notes_have_a_limit_and_no_control_characters_but_line_breaks() {
        let mut deck = fine();
        deck.notes = "z".repeat(MAX_DECK_NOTES_CHARS);
        assert!(check_body(&deck).is_ok());
        deck.notes = "z".repeat(MAX_DECK_NOTES_CHARS + 1);
        assert!(check_body(&deck).is_err());
        for bad in ["a\u{0}b", "a\u{1b}b", "a\u{7f}b"] {
            deck.notes = bad.to_owned();
            let error = check_body(&deck).unwrap_err();
            assert!(error.message().contains("control"), "{}", error.message());
        }
    }

    #[rstest]
    #[case("commander", "commander")]
    #[case("Commander", "commander")]
    #[case(" FREEFORM ", "freeform")]
    #[case("pauper", "pauper")]
    fn formats_come_out_in_lower_case(#[case] sent: &str, #[case] kept: &str) {
        assert_eq!(check_format(sent).unwrap(), kept);
    }

    #[rstest]
    #[case("")]
    #[case("brawl")]
    #[case("com mander")]
    #[case("commander;")]
    fn other_formats_are_refused_with_the_list_of_those_that_exist(#[case] sent: &str) {
        let error = check_format(sent).unwrap_err();
        assert_eq!(error.code(), "invalid_deck");
        assert!(
            error.message().contains("commander, freeform"),
            "{}",
            error.message()
        );
    }

    #[rstest]
    #[case::zero(0)]
    #[case::too_many(1000)]
    fn counts_are_one_to_999(#[case] count: u32) {
        let mut deck = fine();
        deck.cards = vec![line(DeckZone::Main, A, None, count)];
        let error = check_body(&deck).unwrap_err();
        assert!(
            error.message().starts_with("line 1: the count is 1 to 999"),
            "{}",
            error.message()
        );
    }

    #[rstest]
    #[case("")]
    #[case("not-an-id")]
    #[case("4457ed35-7c10-48c8-9776-456485fdf07")]
    #[case("4457ed35-7c10-48c8-9776-456485fdf070-")]
    fn oracle_ids_must_be_ids(#[case] id: &str) {
        let mut deck = fine();
        deck.cards = vec![
            line(DeckZone::Main, A, None, 1),
            line(DeckZone::Main, id, None, 1),
        ];
        let error = check_body(&deck).unwrap_err();
        assert!(
            error.message().starts_with("line 2: oracle_id"),
            "{}",
            error.message()
        );
    }

    #[test]
    fn printing_ids_must_be_ids_too() {
        let mut deck = fine();
        deck.cards = vec![line(DeckZone::Main, A, Some("art"), 1)];
        let error = check_body(&deck).unwrap_err();
        assert!(
            error.message().starts_with("line 1: printing_id"),
            "{}",
            error.message()
        );
    }

    #[test]
    fn the_number_of_lines_and_of_cards_is_limited() {
        let mut deck = fine();
        deck.cards = (0..MAX_DECK_LINES)
            .map(|_| line(DeckZone::Main, A, None, 1))
            .collect();
        assert!(check_body(&deck).is_ok());
        deck.cards.push(line(DeckZone::Main, A, None, 1));
        assert!(check_body(&deck).unwrap_err().message().contains("lines"));

        deck.cards = vec![line(DeckZone::Main, A, None, MAX_DECK_CARDS)];
        assert!(check_body(&deck).is_ok());
        deck.cards.push(line(DeckZone::Sideboard, A, None, 1));
        assert!(check_body(&deck).unwrap_err().message().contains("cards"));
        let huge = vec![line(DeckZone::Main, A, None, 999); 100];
        assert!(
            check_lines(&huge).is_err(),
            "the sum cannot overflow its way past the limit"
        );
    }

    #[test]
    fn lines_convert_to_what_the_store_and_the_check_take_and_back() {
        let line = Line {
            zone: DeckZone::Companion,
            oracle_id: A.to_owned(),
            printing_id: Some(P.to_owned()),
            count: 3,
        };
        assert_eq!(Line::from_stored(&line.to_stored()), line);
        let check = line.to_check();
        assert_eq!(
            (check.zone, check.oracle_id.as_str(), check.count),
            (DeckZone::Companion, A, 3)
        );
        for zone in DeckZone::ALL {
            assert_eq!(zone_of(stored_zone(zone)), zone);
        }
    }
}
