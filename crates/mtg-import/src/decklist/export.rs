//! Plain-text exporter whose output always parses back to the same deck.

use thiserror::Error;

use super::line::{LineOptions, is_collector_number, is_set_code, parse_entry};
use super::parse;
use super::section::header_word;
use super::types::{DeckEntry, MAX_COUNT, ParsedDeck, Section};

/// Why a deck cannot be written as text that reads back identically.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ExportError {
    /// The deck still carries parse errors, which the text form cannot represent.
    #[error("deck has {count} unresolved line errors")]
    HasLineErrors {
        /// Number of line errors.
        count: usize,
    },
    /// One entry cannot be written unambiguously.
    #[error("entry {index} ({name:?}): {reason}")]
    InvalidEntry {
        /// Zero-based entry position.
        index: usize,
        /// The entry's card name.
        name: String,
        /// Why the entry cannot be exported.
        reason: String,
    },
    /// The assembled text reads back differently; this guards against unforeseen ambiguity.
    #[error("exported text would not parse back to the same deck")]
    NotRoundTrippable,
}

/// Formats `entry` as a line and verifies the line reads back as exactly `entry` (section aside).
fn entry_line(entry: &DeckEntry) -> Result<String, String> {
    if !(1..=MAX_COUNT).contains(&entry.count) {
        return Err(format!("count {} is outside 1..={MAX_COUNT}", entry.count));
    }
    let mut line = format!("{} {}", entry.count, entry.name);
    match (&entry.set, &entry.collector_number) {
        (None, None) => {}
        (None, Some(_)) => return Err("a collector number requires a set".to_owned()),
        (Some(set), number) => {
            if !is_set_code(set) || *set != set.to_ascii_uppercase() {
                return Err(format!("{set:?} is not an uppercase set code"));
            }
            line = format!("{line} ({set})");
            if let Some(number) = number {
                if !is_collector_number(number) || number.contains(char::is_whitespace) {
                    return Err(format!("{number:?} is not a collector number"));
                }
                line = format!("{line} {number}");
            }
        }
    }
    let options = LineOptions {
        bracket_sets: true,
        categories: false,
    };
    let read = parse_entry(&line, options).map_err(|e| e.to_string())?;
    let same = read.count == entry.count
        && read.name == entry.name
        && read.set == entry.set
        && read.collector_number == entry.collector_number
        && read.category.is_none()
        && !read.foil;
    if same {
        Ok(line)
    } else {
        Err("name or printing would be misread as something else".to_owned())
    }
}

/// Writes `deck` as plain text with a header at each section change, or fails if it would not parse back.
pub fn export(deck: &ParsedDeck) -> Result<String, ExportError> {
    if !deck.errors.is_empty() {
        return Err(ExportError::HasLineErrors {
            count: deck.errors.len(),
        });
    }
    let mut text = String::new();
    let mut current = None;
    for (index, entry) in deck.entries.iter().enumerate() {
        let line = entry_line(entry).map_err(|reason| ExportError::InvalidEntry {
            index,
            name: entry.name.clone(),
            reason,
        })?;
        if current != Some(entry.section) {
            // A leading main deck needs no header, which keeps simple lists MTGO-compatible.
            if current.is_some() {
                text.push('\n');
            }
            if current.is_some() || entry.section != Section::Main {
                text.push_str(header_word(entry.section));
                text.push('\n');
            }
            current = Some(entry.section);
        }
        text.push_str(&line);
        text.push('\n');
    }
    if parse(&text) == *deck {
        Ok(text)
    } else {
        Err(ExportError::NotRoundTrippable)
    }
}

#[cfg(test)]
mod tests {
    //! Goal: for every deck the exporter accepts, parse(export(d)) == d, and anything that could not survive the trip is refused instead of corrupted.
    use super::*;
    use crate::decklist::{DeckFormat, LineError, detect};
    use proptest::prelude::*;
    use rstest::rstest;

    fn deck(entries: Vec<DeckEntry>) -> ParsedDeck {
        ParsedDeck {
            entries,
            errors: Vec::new(),
        }
    }

    fn e(count: u32, name: &str, section: Section) -> DeckEntry {
        DeckEntry::new(count, name, section)
    }

    #[rstest]
    #[case::empty(vec![], "")]
    #[case::main_only(vec![e(4, "Lightning Bolt", Section::Main), e(2, "Shock", Section::Main)], "4 Lightning Bolt\n2 Shock\n")]
    #[case::with_printing(vec![e(4, "Lightning Bolt", Section::Main).with_printing("m10", Some("146"))], "4 Lightning Bolt (M10) 146\n")]
    #[case::set_only(vec![e(1, "Sol Ring", Section::Main).with_printing("CMM", None)], "1 Sol Ring (CMM)\n")]
    #[case::sideboard(vec![e(4, "Bolt", Section::Main), e(2, "Shock", Section::Sideboard)], "4 Bolt\n\nSideboard\n2 Shock\n")]
    #[case::commander_first(
        vec![e(1, "Atraxa", Section::Commander), e(99, "Forest", Section::Main)],
        "Commander\n1 Atraxa\n\nDeck\n99 Forest\n"
    )]
    #[case::sideboard_first(vec![e(1, "A", Section::Sideboard), e(1, "B", Section::Main)], "Sideboard\n1 A\n\nDeck\n1 B\n")]
    #[case::interleaved(
        vec![e(1, "A", Section::Main), e(1, "B", Section::Sideboard), e(1, "C", Section::Main)],
        "1 A\n\nSideboard\n1 B\n\nDeck\n1 C\n"
    )]
    #[case::all_sections(
        vec![
            e(1, "Co", Section::Companion),
            e(1, "Cm", Section::Commander),
            e(1, "Ma", Section::Main),
            e(1, "Sb", Section::Sideboard),
            e(1, "Mb", Section::Maybeboard),
        ],
        "Companion\n1 Co\n\nCommander\n1 Cm\n\nDeck\n1 Ma\n\nSideboard\n1 Sb\n\nMaybeboard\n1 Mb\n"
    )]
    #[case::duplicates_kept(vec![e(1, "A", Section::Main), e(1, "A", Section::Main)], "1 A\n1 A\n")]
    #[case::split_and_parens_in_name(vec![e(1, "Fire // Ice", Section::Main), e(1, "B.F.M. (Big Furry Monster)", Section::Main)], "1 Fire // Ice\n1 B.F.M. (Big Furry Monster)\n")]
    fn writes_expected_text_and_reads_it_back(
        #[case] entries: Vec<DeckEntry>,
        #[case] expected: &str,
    ) {
        let original = deck(entries);
        let text = export(&original).unwrap();
        assert_eq!(text, expected);
        assert_eq!(crate::decklist::parse(&text), original);
    }

    #[rstest]
    #[case::zero_count(e(0, "A", Section::Main), "count")]
    #[case::huge_count(e(1000, "A", Section::Main), "count")]
    #[case::empty_name(e(1, "", Section::Main), "missing card name")]
    #[case::padded_name(e(1, " A", Section::Main), "misread")]
    #[case::trailing_space(e(1, "A ", Section::Main), "misread")]
    #[case::newline_in_name(e(1, "A\nB", Section::Main), "control")]
    #[case::foil_marker_in_name(e(1, "A *F*", Section::Main), "misread")]
    #[case::bracket_set_in_name(e(1, "A [M10]", Section::Main), "misread")]
    #[case::paren_set_in_name(e(1, "Hazmat Suit (Used)", Section::Main), "misread")]
    #[case::lowercase_set(DeckEntry { set: Some("m10".into()), ..e(1, "A", Section::Main) }, "uppercase set")]
    #[case::bad_set(DeckEntry { set: Some("M".into()), ..e(1, "A", Section::Main) }, "uppercase set")]
    #[case::number_without_set(DeckEntry { collector_number: Some("1".into()), ..e(1, "A", Section::Main) }, "requires a set")]
    #[case::spaced_number(DeckEntry { set: Some("M10".into()), collector_number: Some("1 2".into()), ..e(1, "A", Section::Main) }, "collector number")]
    #[case::digitless_number(DeckEntry { set: Some("M10".into()), collector_number: Some("abc".into()), ..e(1, "A", Section::Main) }, "collector number")]
    fn refuses_entries_that_cannot_round_trip(#[case] entry: DeckEntry, #[case] fragment: &str) {
        let error = export(&deck(vec![e(1, "Fine", Section::Main), entry])).unwrap_err();
        let message = error.to_string();
        assert!(
            matches!(error, ExportError::InvalidEntry { index: 1, .. }),
            "{message}"
        );
        assert!(message.contains(fragment), "{message}");
    }

    #[test]
    fn refuses_decks_with_line_errors() {
        let mut broken = deck(vec![e(1, "A", Section::Main)]);
        broken.errors.push(LineError {
            line: 3,
            message: "bad".into(),
        });
        assert_eq!(
            export(&broken).unwrap_err(),
            ExportError::HasLineErrors { count: 1 }
        );
    }

    #[rstest]
    #[case::plain("4 Lightning Bolt (M10) 146\n2 Shock\n\n3 Duress\n")]
    #[case::arena("Deck\n4 Lightning Bolt (M10) 146\n\nSideboard\n2 Pyroblast (ICE) 213\n")]
    #[case::commander(
        "Commander\n1 Atraxa, Praetors' Voice (C16) 28\n\nDeck\n1 Sol Ring (CMM) 400\n"
    )]
    #[case::crlf("4 Bolt\r\n\r\n2 Shock\r\n")]
    fn exporting_a_parsed_deck_is_stable(#[case] input: &str) {
        let parsed = crate::decklist::parse(input);
        assert_eq!(parsed.errors, Vec::new());
        let once = export(&parsed).unwrap();
        let twice = export(&crate::decklist::parse(&once)).unwrap();
        assert_eq!(once, twice);
    }

    fn section() -> impl Strategy<Value = Section> {
        prop_oneof![
            Just(Section::Main),
            Just(Section::Sideboard),
            Just(Section::Commander),
            Just(Section::Companion),
            Just(Section::Maybeboard),
        ]
    }

    fn entry() -> impl Strategy<Value = DeckEntry> {
        let name = "[A-Za-z0-9][A-Za-z0-9 ,'/:!?&.\u{e9}\u{fb}\u{c6}-]{0,30}[A-Za-z0-9]|[A-Za-z]";
        let printing = prop::option::of((
            "[A-Z0-9]{2,6}",
            prop::option::of("[A-Z]?[0-9]{1,3}[a-z\u{2605}]?"),
        ));
        (1..=MAX_COUNT, name, printing, section()).prop_map(|(count, name, printing, section)| {
            let mut entry = DeckEntry::new(count, name, section);
            if let Some((set, number)) = printing {
                entry = entry.with_printing(&set, number.as_deref());
            }
            entry
        })
    }

    proptest! {
        #[test]
        fn parse_of_export_equals_the_deck(entries in prop::collection::vec(entry(), 0..30)) {
            let original = deck(entries);
            let text = export(&original).unwrap();
            prop_assert_eq!(crate::decklist::parse(&text), original);
        }

        #[test]
        fn exported_text_is_never_misdetected_as_a_richer_format(entries in prop::collection::vec(entry(), 1..20)) {
            let text = export(&deck(entries)).unwrap();
            prop_assert!(matches!(detect(&text), DeckFormat::PlainText | DeckFormat::Arena));
        }

        #[test]
        fn hostile_entries_are_refused_or_survive(
            count in any::<u32>(),
            name in "\\PC{0,40}",
            set in prop::option::of("\\PC{0,8}"),
            number in prop::option::of("\\PC{0,8}"),
            section in section(),
        ) {
            let original = deck(vec![DeckEntry { count, name, set, collector_number: number, section }]);
            if let Ok(text) = export(&original) {
                prop_assert_eq!(crate::decklist::parse(&text), original);
            }
        }
    }
}
