//! Pasted lists and shared lists: reading a list in any format `mtg-import` knows and tying its names
//! to cards of the database, and writing a stored deck as text, Arena or MTGO text.

use axum::http::StatusCode;
use mtg_cards::deck::DeckZone;
use mtg_db::StoredDeck;
use mtg_import::decklist::{
    DeckEntry, DeckFormat, ExportFormat, ParsedDeck, Section, detect, export_as, parse_as,
    promote_implied_commander,
};
use mtg_import::resolve::{ResolveError, SUGGESTION_LIMIT, resolve_deck};
use mtg_import::scryfall::Layout;
use mtg_protocol::rest::{
    CardSuggestion, DeckErrorCode, IgnoredCards, ImportRequest, ImportResult, ImportSource,
    ListProblem, MAX_IMPORT_CHARS, MAX_IMPORT_LINES, UnresolvedLine, UnresolvedReason,
};

use crate::api::ApiError;
use crate::card_api::failed;
use crate::cards::CardSource;
use crate::deck_cards::{Line, Loaded, check_format};

fn dialect(source: ImportSource) -> DeckFormat {
    match source {
        ImportSource::Text => DeckFormat::PlainText,
        ImportSource::Arena => DeckFormat::Arena,
        ImportSource::Mtgo => DeckFormat::MtgoText,
        ImportSource::MtgoDek => DeckFormat::MtgoDek,
        ImportSource::Moxfield => DeckFormat::Moxfield,
        ImportSource::Archidekt => DeckFormat::Archidekt,
    }
}

fn source_of(format: DeckFormat) -> ImportSource {
    match format {
        DeckFormat::PlainText => ImportSource::Text,
        DeckFormat::Arena => ImportSource::Arena,
        DeckFormat::MtgoText => ImportSource::Mtgo,
        DeckFormat::MtgoDek => ImportSource::MtgoDek,
        DeckFormat::Moxfield => ImportSource::Moxfield,
        DeckFormat::Archidekt => ImportSource::Archidekt,
    }
}

fn zone_of(section: Section) -> Option<DeckZone> {
    match section {
        Section::Main => Some(DeckZone::Main),
        Section::Sideboard => Some(DeckZone::Sideboard),
        Section::Commander => Some(DeckZone::Commander),
        Section::Companion => Some(DeckZone::Companion),
        Section::Maybeboard => None,
    }
}

fn section_of(zone: DeckZone) -> Section {
    match zone {
        DeckZone::Main => Section::Main,
        DeckZone::Sideboard => Section::Sideboard,
        DeckZone::Commander => Section::Commander,
        DeckZone::Companion => Section::Companion,
    }
}

fn too_big(message: String) -> ApiError {
    ApiError::new(
        StatusCode::BAD_REQUEST,
        DeckErrorCode::InvalidRequest.as_str(),
        message,
    )
}

/// The list of a request read into entries: the format it was read as, and the entries (with the
/// maybeboard counted apart, since a stored deck has none).
fn read(request: &ImportRequest) -> Result<(DeckFormat, ParsedDeck, u64), ApiError> {
    if request.text.chars().count() > MAX_IMPORT_CHARS {
        return Err(too_big(format!(
            "the list is at most {MAX_IMPORT_CHARS} characters"
        )));
    }
    if request.text.lines().count() > MAX_IMPORT_LINES {
        return Err(too_big(format!(
            "the list is at most {MAX_IMPORT_LINES} lines"
        )));
    }
    let wanted = request
        .format
        .as_deref()
        .map(check_format)
        .transpose()?
        .unwrap_or_default();
    let format = request
        .source
        .map_or_else(|| detect(&request.text), dialect);
    let mut parsed = parse_as(format, &request.text);
    if wanted == "commander" {
        parsed = promote_implied_commander(format, parsed);
    }
    let maybe: u64 = parsed
        .entries
        .iter()
        .filter(|entry| entry.section == Section::Maybeboard)
        .map(|entry| u64::from(entry.count))
        .sum();
    parsed
        .entries
        .retain(|entry| entry.section != Section::Maybeboard);
    Ok((format, parsed, maybe))
}

/// The printing a line names by set and collector number, when the database holds it and it is a
/// printing of the card the name resolved to.
async fn printing_of(
    cards: &CardSource,
    set: Option<&str>,
    number: Option<&str>,
    oracle_id: &str,
) -> Result<Option<String>, ApiError> {
    let (Some(set), Some(number)) = (set, number) else {
        return Ok(None);
    };
    let found = cards
        .database()
        .cards()
        .printing_by_number(set, number)
        .await
        .map_err(|error| failed(&error))?;
    Ok(found
        .filter(|printing| printing.oracle_id.as_str() == oracle_id)
        .map(|printing| printing.printing_id.to_string()))
}

async fn unresolved_line(
    cards: &CardSource,
    entry: &DeckEntry,
    error: ResolveError,
) -> Result<Option<UnresolvedLine>, ApiError> {
    let Some(zone) = zone_of(entry.section) else {
        return Ok(None);
    };
    let (reason, message, suggestions) = match error {
        ResolveError::UnknownCard { name, .. } => {
            let found = cards
                .database()
                .cards()
                .suggest(&name, u32::try_from(SUGGESTION_LIMIT).unwrap_or(5))
                .await
                .map_err(|error| failed(&error))?;
            (
                UnresolvedReason::UnknownCard,
                format!("no card is named {name:?}"),
                found
                    .into_iter()
                    .map(|s| CardSuggestion {
                        oracle_id: s.oracle_id.to_string(),
                        name: s.name,
                    })
                    .collect(),
            )
        }
        ResolveError::AmbiguousName { name, candidates } => (
            UnresolvedReason::AmbiguousName,
            format!("several cards are named {name:?}"),
            candidates
                .into_iter()
                .map(|s| CardSuggestion {
                    oracle_id: s.oracle_id.to_string(),
                    name: s.name,
                })
                .collect(),
        ),
    };
    Ok(Some(UnresolvedLine {
        name: entry.name.clone(),
        count: entry.count,
        zone,
        reason,
        message,
        suggestions,
    }))
}

/// Reads a pasted list and ties its names to cards. Names are matched exactly (case, accents and
/// the way split cards are written do not matter) and never guessed: a name that matches no card or
/// several is reported with suggestions. Nothing is saved.
pub(crate) async fn import(
    cards: &CardSource,
    request: &ImportRequest,
) -> Result<ImportResult, ApiError> {
    let (format, parsed, maybeboard) = read(request)?;
    let index = cards.card_index().await.map_err(|error| failed(&error))?;
    let resolution = resolve_deck(&index, &parsed);

    let mut lines = Vec::with_capacity(resolution.resolved.len());
    for entry in &resolution.resolved {
        let Some(zone) = zone_of(entry.section) else {
            continue;
        };
        let oracle_id = entry.oracle_id.to_string();
        let printing_id = printing_of(
            cards,
            entry.set.as_deref(),
            entry.collector_number.as_deref(),
            &oracle_id,
        )
        .await?;
        lines.push(Line {
            zone,
            oracle_id,
            printing_id,
            count: entry.count,
        });
    }
    let loaded = Loaded::load(cards, &lines).await?;
    let mut views = Vec::with_capacity(lines.len());
    for line in &lines {
        if let Some(view) = loaded.view(cards, line)? {
            views.push(view);
        }
    }

    let mut unresolved = Vec::new();
    for item in resolution.unresolved {
        if let Some(line) = unresolved_line(cards, &item.entry, item.error).await? {
            unresolved.push(line);
        }
    }
    let mut ignored: Vec<IgnoredCards> = parsed
        .ignored
        .iter()
        .map(|section| IgnoredCards {
            name: section.name.clone(),
            count: section.count,
        })
        .collect();
    if maybeboard > 0 {
        ignored.push(IgnoredCards {
            name: "Maybeboard".to_owned(),
            count: maybeboard,
        });
    }
    Ok(ImportResult {
        source: source_of(format),
        cards: views,
        unresolved,
        problems: parsed
            .errors
            .iter()
            .map(|error| ListProblem {
                line: u32::try_from(error.line).unwrap_or(u32::MAX),
                message: error.message.clone(),
            })
            .collect(),
        ignored,
    })
}

/// How a format spells a card with several faces: split cards as the format writes them, other
/// multi-faced cards by their front face (as Arena and MTGO do), and text by the full name.
fn written_name(card: &mtg_db::Card, format: ExportFormat) -> String {
    let faces: Vec<&str> = card.faces.iter().map(|face| face.name.as_str()).collect();
    if faces.len() < 2 || format == ExportFormat::Text {
        return card.name.clone();
    }
    match (format, &card.layout) {
        (ExportFormat::Arena, Layout::Split) => faces.join(" /// "),
        (ExportFormat::Mtgo, Layout::Split) => faces.join("/"),
        _ => faces[0].to_owned(),
    }
}

fn entries(
    deck: &StoredDeck,
    loaded: &Loaded,
    format: ExportFormat,
    printings: bool,
) -> Vec<DeckEntry> {
    deck.lines
        .iter()
        .map(Line::from_stored)
        .filter_map(|line| {
            let card = loaded.card(&line.oracle_id)?;
            let mut entry = DeckEntry::new(
                line.count,
                written_name(card, format),
                section_of(line.zone),
            );
            if let Some(printing) = printings
                .then_some(line.printing_id.as_deref())
                .flatten()
                .and_then(|id| loaded.printing(id))
            {
                entry =
                    entry.with_printing(&printing.set, Some(printing.collector_number.as_str()));
            }
            Some(entry)
        })
        .collect()
}

/// A stored deck as text in `format`. Printings are written where the format has them and the
/// deck's printings can be written as the format reads them; otherwise the names alone.
pub(crate) fn write(
    deck: &StoredDeck,
    loaded: &Loaded,
    format: ExportFormat,
) -> Result<String, ApiError> {
    let parsed = |printings| ParsedDeck {
        entries: entries(deck, loaded, format, printings),
        ..ParsedDeck::default()
    };
    export_as(format, &parsed(true))
        .or_else(|_| export_as(format, &parsed(false)))
        .map_err(|error| {
            ApiError::new(
                StatusCode::UNPROCESSABLE_ENTITY,
                DeckErrorCode::ExportFailed.as_str(),
                format!(
                    "the deck cannot be written as {}: {error}",
                    format.keyword()
                ),
            )
        })
}

/// A file name for a deck: its name in ASCII letters, digits and dashes.
pub(crate) fn file_stem(name: &str) -> String {
    let mut stem = String::new();
    for c in name.chars() {
        if c.is_ascii_alphanumeric() {
            stem.push(c.to_ascii_lowercase());
        } else if !stem.ends_with('-') && !stem.is_empty() {
            stem.push('-');
        }
    }
    let stem = stem.trim_end_matches('-');
    if stem.is_empty() {
        "deck".to_owned()
    } else {
        stem.chars().take(40).collect()
    }
}

#[cfg(test)]
mod tests {
    //! Goal: a pasted list is read as the format it says or the one it looks like, with the maybeboard and the sticker sections counted apart and the list's size bounded; file names for downloads are plain ASCII; the names a list uses are looked up once each, in the form the database knows.
    use rstest::rstest;

    use super::*;

    fn request(text: &str) -> ImportRequest {
        ImportRequest {
            text: text.to_owned(),
            source: None,
            format: None,
        }
    }

    #[test]
    fn the_format_is_detected_unless_the_request_names_it() {
        let text = "Commander\n1 Atraxa, Praetors' Voice\n\nDeck\n1 Sol Ring\n";
        let (format, parsed, maybe) = read(&request(text)).unwrap();
        assert_eq!(format, DeckFormat::Arena);
        assert_eq!(parsed.entries.len(), 2);
        assert_eq!(maybe, 0);
        let named = ImportRequest {
            source: Some(ImportSource::Text),
            ..request(text)
        };
        let (format, _, _) = read(&named).unwrap();
        assert_eq!(format, DeckFormat::PlainText);
    }

    #[test]
    fn a_commander_deck_from_moxfield_promotes_the_first_line() {
        let text = "1 Najeela, the Blade-Blossom (BBD) 62 *F*\n1 Ad Nauseam (ALA) 63\n";
        let plain = read(&request(text)).unwrap().1;
        assert_eq!(plain.entries[0].section, Section::Main);
        let commander = ImportRequest {
            format: Some("Commander".into()),
            ..request(text)
        };
        let (format, promoted, _) = read(&commander).unwrap();
        assert_eq!(format, DeckFormat::Moxfield);
        assert_eq!(promoted.entries[0].section, Section::Commander);
        let modern = ImportRequest {
            format: Some("modern".into()),
            ..request(text)
        };
        assert_eq!(read(&modern).unwrap().1.entries[0].section, Section::Main);
    }

    #[test]
    fn the_maybeboard_is_counted_apart_and_is_no_entry() {
        let text = "Deck\n4 Shock\n\nMaybeboard\n2 Pyroblast\n1 Sol Ring\n";
        let (_, parsed, maybe) = read(&request(text)).unwrap();
        assert_eq!(maybe, 3);
        assert_eq!(parsed.entries.len(), 1);
        assert!(parsed.entries.iter().all(|e| e.section == Section::Main));
    }

    #[test]
    fn the_size_of_a_list_is_bounded() {
        let long = "x".repeat(MAX_IMPORT_CHARS + 1);
        assert!(read(&request(&long)).is_err());
        let many = "1 Shock\n".repeat(MAX_IMPORT_LINES + 1);
        let error = read(&request(&many)).unwrap_err();
        assert_eq!(error.code(), "invalid_request");
        assert!(read(&request(&"1 Shock\n".repeat(MAX_IMPORT_LINES))).is_ok());
        let bad_format = ImportRequest {
            format: Some("brawl".into()),
            ..request("1 Shock")
        };
        assert_eq!(read(&bad_format).unwrap_err().code(), "invalid_deck");
    }

    #[rstest]
    #[case("My Burn Deck", "my-burn-deck")]
    #[case("  --Weird__Name!!  ", "weird-name")]
    #[case("Æther & Co.", "ther-co")]
    #[case("日本語", "deck")]
    #[case("", "deck")]
    #[case("a/b\\c\"d", "a-b-c-d")]
    fn download_names_are_plain_ascii(#[case] name: &str, #[case] stem: &str) {
        assert_eq!(file_stem(name), stem);
    }

    #[test]
    fn download_names_are_short() {
        assert_eq!(file_stem(&"abc ".repeat(50)).len(), 40);
    }
}
