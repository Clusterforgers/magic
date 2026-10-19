//! Writers for the formats a deck is shared in: plain text, the Arena export and MTGO text. Each
//! output is read back through the parser of its own dialect before it is returned, so a writer
//! never produces text that its parser reads as another deck.

use super::export::{ExportError, entry_line, export};
use super::types::{DeckEntry, DeckFormat, ParsedDeck, Section};
use super::{parse_as, section};

/// A format [`export_as`] writes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ExportFormat {
    /// Plain text: `N Name (SET) number` lines, headers where the section changes.
    Text,
    /// The Arena export: `Commander`, `Companion`, `Deck` and `Sideboard` sections, printings kept.
    Arena,
    /// MTGO text: names only, a `SIDEBOARD:` section, the commander in a last block of its own.
    Mtgo,
}

impl ExportFormat {
    /// The name of the format in URLs and requests.
    pub const fn keyword(self) -> &'static str {
        match self {
            ExportFormat::Text => "text",
            ExportFormat::Arena => "arena",
            ExportFormat::Mtgo => "mtgo",
        }
    }

    /// Reads a [`keyword`](Self::keyword).
    pub fn from_keyword(keyword: &str) -> Option<ExportFormat> {
        [ExportFormat::Text, ExportFormat::Arena, ExportFormat::Mtgo]
            .into_iter()
            .find(|format| format.keyword() == keyword)
    }
}

/// The entries of `deck` that belong to `sections`, section by section, each in the order it was given.
fn arranged(deck: &ParsedDeck, sections: &[Section]) -> Vec<DeckEntry> {
    sections
        .iter()
        .flat_map(|section| deck.entries.iter().filter(move |e| e.section == *section))
        .cloned()
        .collect()
}

/// Appends a block of lines; blocks are told apart by one blank line.
fn push_block(text: &mut String, header: Option<&str>, lines: &[String]) {
    if !text.is_empty() {
        text.push('\n');
    }
    if let Some(header) = header {
        text.push_str(header);
        text.push('\n');
    }
    for line in lines {
        text.push_str(line);
        text.push('\n');
    }
}

fn lines_of(entries: &[DeckEntry], from: usize) -> Result<Vec<String>, ExportError> {
    entries
        .iter()
        .enumerate()
        .map(|(index, entry)| {
            entry_line(entry).map_err(|reason| ExportError::InvalidEntry {
                index: from + index,
                name: entry.name.clone(),
                reason,
            })
        })
        .collect()
}

/// The text of `deck` as the Arena client writes one, with its printings: `Commander`, `Companion`,
/// `Deck` and `Sideboard` sections in that order, each only if it holds cards.
fn arena(deck: &ParsedDeck) -> Result<(String, Vec<DeckEntry>), ExportError> {
    let order = [
        Section::Commander,
        Section::Companion,
        Section::Main,
        Section::Sideboard,
    ];
    let entries = arranged(deck, &order);
    let mut text = String::new();
    let mut from = 0;
    for part in order {
        let block: Vec<DeckEntry> = entries
            .iter()
            .filter(|e| e.section == part)
            .cloned()
            .collect();
        if block.is_empty() {
            continue;
        }
        push_block(
            &mut text,
            Some(section::header_word(part)),
            &lines_of(&block, from)?,
        );
        from += block.len();
    }
    Ok((text, entries))
}

/// The text of `deck` as MTGO takes it: names only (a companion goes to the sideboard, where MTGO
/// keeps it), the main deck, a `SIDEBOARD:` block when there is a sideboard, and the commanders in a last
/// block with no header.
fn mtgo(deck: &ParsedDeck) -> Result<(String, Vec<DeckEntry>), ExportError> {
    let plain = |entry: &DeckEntry, section| DeckEntry {
        count: entry.count,
        name: entry.name.clone(),
        set: None,
        collector_number: None,
        section,
    };
    let pick = |wanted: &[Section], section| -> Vec<DeckEntry> {
        arranged(deck, wanted)
            .iter()
            .map(|entry| plain(entry, section))
            .collect()
    };
    let main = pick(&[Section::Main], Section::Main);
    let side = pick(
        &[Section::Sideboard, Section::Companion],
        Section::Sideboard,
    );
    let command = pick(&[Section::Commander], Section::Commander);

    let mut text = String::new();
    let mut from = 0;
    if !main.is_empty() {
        push_block(&mut text, None, &lines_of(&main, from)?);
        from += main.len();
    }
    if !side.is_empty() {
        push_block(&mut text, Some("SIDEBOARD:"), &lines_of(&side, from)?);
        from += side.len();
    }
    if !command.is_empty() {
        push_block(&mut text, None, &lines_of(&command, from)?);
    }
    Ok((text, [main, side, command].concat()))
}

/// Writes `deck` in `format`. The text is read back with the parser of the format and must give the
/// deck the format can hold: Arena and MTGO list the sections in their own order and have no
/// maybeboard, MTGO has no printings and no companion zone.
pub fn export_as(format: ExportFormat, deck: &ParsedDeck) -> Result<String, ExportError> {
    if !deck.errors.is_empty() {
        return Err(ExportError::HasLineErrors {
            count: deck.errors.len(),
        });
    }
    let (text, expected, dialect) = match format {
        ExportFormat::Text => return export(deck),
        ExportFormat::Arena => {
            let (text, entries) = arena(deck)?;
            (text, entries, DeckFormat::Arena)
        }
        ExportFormat::Mtgo => {
            let (text, entries) = mtgo(deck)?;
            (text, entries, DeckFormat::MtgoText)
        }
    };
    let again = parse_as(dialect, &text);
    if again.entries == expected && again.errors.is_empty() && again.ignored.is_empty() {
        Ok(text)
    } else {
        Err(ExportError::NotRoundTrippable)
    }
}

#[cfg(test)]
mod tests {
    //! Goal: each writer produces the layout of its format and reads back, through the parser of that format, as exactly the deck the format can hold; what a format cannot hold is dropped on purpose, and what would be misread is refused.
    use proptest::prelude::*;
    use rstest::rstest;

    use super::*;
    use crate::decklist::{LineError, MAX_COUNT, detect, parse};

    fn deck(entries: Vec<DeckEntry>) -> ParsedDeck {
        ParsedDeck {
            entries,
            ..ParsedDeck::default()
        }
    }

    fn e(count: u32, name: &str, section: Section) -> DeckEntry {
        DeckEntry::new(count, name, section)
    }

    fn printed(entry: DeckEntry, set: &str, number: &str) -> DeckEntry {
        entry.with_printing(set, Some(number))
    }

    #[rstest]
    #[case::keywords(ExportFormat::Text, "text")]
    #[case::arena(ExportFormat::Arena, "arena")]
    #[case::mtgo(ExportFormat::Mtgo, "mtgo")]
    fn formats_are_named_by_their_keyword(#[case] format: ExportFormat, #[case] keyword: &str) {
        assert_eq!(format.keyword(), keyword);
        assert_eq!(ExportFormat::from_keyword(keyword), Some(format));
    }

    #[rstest]
    #[case("")]
    #[case("Text")]
    #[case("dek")]
    #[case("moxfield")]
    fn other_names_are_no_format(#[case] keyword: &str) {
        assert_eq!(ExportFormat::from_keyword(keyword), None);
    }

    #[test]
    fn text_is_the_plain_text_exporter() {
        let d = deck(vec![
            e(4, "Bolt", Section::Main),
            e(2, "Shock", Section::Sideboard),
        ]);
        assert_eq!(
            export_as(ExportFormat::Text, &d).unwrap(),
            "4 Bolt\n\nSideboard\n2 Shock\n"
        );
    }

    #[rstest]
    #[case::main_only(
        vec![e(4, "Lightning Bolt", Section::Main)],
        "Deck\n4 Lightning Bolt\n"
    )]
    #[case::printings_are_kept(
        vec![printed(e(4, "Lightning Bolt", Section::Main), "m10", "146")],
        "Deck\n4 Lightning Bolt (M10) 146\n"
    )]
    #[case::sideboard(
        vec![e(4, "Bolt", Section::Main), e(2, "Shock", Section::Sideboard)],
        "Deck\n4 Bolt\n\nSideboard\n2 Shock\n"
    )]
    #[case::commander_first(
        vec![e(99, "Forest", Section::Main), e(1, "Atraxa", Section::Commander)],
        "Commander\n1 Atraxa\n\nDeck\n99 Forest\n"
    )]
    #[case::every_zone_in_arena_order(
        vec![
            e(1, "Sb", Section::Sideboard),
            e(1, "Ma", Section::Main),
            e(1, "Co", Section::Companion),
            e(1, "Cm", Section::Commander),
        ],
        "Commander\n1 Cm\n\nCompanion\n1 Co\n\nDeck\n1 Ma\n\nSideboard\n1 Sb\n"
    )]
    #[case::split_cards_and_parentheses(
        vec![e(1, "Fire // Ice", Section::Main), e(1, "B.F.M. (Big Furry Monster)", Section::Main)],
        "Deck\n1 Fire // Ice\n1 B.F.M. (Big Furry Monster)\n"
    )]
    #[case::duplicates_stay_apart(
        vec![e(1, "A", Section::Main), e(1, "A", Section::Main)],
        "Deck\n1 A\n1 A\n"
    )]
    fn arena_writes_the_client_layout_and_reads_back(
        #[case] entries: Vec<DeckEntry>,
        #[case] expected: &str,
    ) {
        let text = export_as(ExportFormat::Arena, &deck(entries.clone())).unwrap();
        assert_eq!(text, expected);
        assert_eq!(detect(&text), DeckFormat::Arena);
        let again = parse(&text);
        assert!(again.errors.is_empty(), "{:?}", again.errors);
        let mut sorted = entries;
        sorted.sort_by_key(|entry| match entry.section {
            Section::Commander => 0,
            Section::Companion => 1,
            Section::Main => 2,
            Section::Sideboard => 3,
            Section::Maybeboard => 4,
        });
        assert_eq!(again.entries, sorted);
    }

    #[rstest]
    #[case::main_only(vec![e(4, "Lightning Bolt", Section::Main)], "4 Lightning Bolt\n")]
    #[case::printings_are_dropped(
        vec![printed(e(4, "Lightning Bolt", Section::Main), "m10", "146")],
        "4 Lightning Bolt\n"
    )]
    #[case::sideboard(
        vec![e(4, "Bolt", Section::Main), e(2, "Shock", Section::Sideboard)],
        "4 Bolt\n\nSIDEBOARD:\n2 Shock\n"
    )]
    #[case::commander_last(
        vec![e(1, "Atraxa", Section::Commander), e(99, "Forest", Section::Main)],
        "99 Forest\n\n1 Atraxa\n"
    )]
    #[case::commander_after_the_sideboard(
        vec![
            e(1, "Atraxa", Section::Commander),
            e(98, "Forest", Section::Main),
            e(1, "Fury", Section::Sideboard),
        ],
        "98 Forest\n\nSIDEBOARD:\n1 Fury\n\n1 Atraxa\n"
    )]
    #[case::a_companion_sits_in_the_sideboard(
        vec![e(60, "Forest", Section::Main), e(1, "Lurrus", Section::Companion), e(1, "Fury", Section::Sideboard)],
        "60 Forest\n\nSIDEBOARD:\n1 Fury\n1 Lurrus\n"
    )]
    #[case::no_main_deck(vec![e(2, "Shock", Section::Sideboard)], "SIDEBOARD:\n2 Shock\n")]
    fn mtgo_writes_names_a_sideboard_block_and_the_commander_last(
        #[case] entries: Vec<DeckEntry>,
        #[case] expected: &str,
    ) {
        let text = export_as(ExportFormat::Mtgo, &deck(entries)).unwrap();
        assert_eq!(text, expected);
        let again = parse_as(DeckFormat::MtgoText, &text);
        assert!(again.errors.is_empty(), "{:?}", again.errors);
        let written = text
            .lines()
            .filter(|line| line.starts_with(|c: char| c.is_ascii_digit()))
            .count();
        assert_eq!(again.entries.len(), written);
    }

    #[test]
    fn mtgo_text_with_a_sideboard_is_recognized_as_mtgo_by_its_header() {
        let text = export_as(
            ExportFormat::Mtgo,
            &deck(vec![
                e(60, "Forest", Section::Main),
                e(1, "Fury", Section::Sideboard),
                e(1, "Atraxa", Section::Commander),
            ]),
        )
        .unwrap();
        assert_eq!(detect(&text), DeckFormat::MtgoText);
        assert_eq!(
            parse(&text).entries,
            parse_as(DeckFormat::MtgoText, &text).entries
        );
    }

    #[test]
    fn the_maybeboard_and_ignored_sections_are_not_written() {
        let mut d = deck(vec![
            e(1, "A", Section::Main),
            e(1, "Maybe", Section::Maybeboard),
        ]);
        d.ignored.push(crate::decklist::IgnoredSection {
            name: "STICKERS".into(),
            count: 2,
        });
        for format in [ExportFormat::Arena, ExportFormat::Mtgo] {
            let text = export_as(format, &d).unwrap();
            assert!(
                !text.contains("Maybe") && !text.contains("STICKERS"),
                "{text}"
            );
        }
    }

    #[test]
    fn a_commander_without_a_main_deck_cannot_be_written_for_mtgo() {
        // MTGO text marks the command zone by the block after the main deck; with none, it reads as the main deck.
        let d = deck(vec![e(1, "Atraxa", Section::Commander)]);
        assert_eq!(
            export_as(ExportFormat::Mtgo, &d),
            Err(ExportError::NotRoundTrippable)
        );
        assert_eq!(
            export_as(ExportFormat::Arena, &d).unwrap(),
            "Commander\n1 Atraxa\n"
        );
    }

    #[rstest]
    #[case::zero_count(e(0, "A", Section::Main), "count")]
    #[case::huge_count(e(1000, "A", Section::Main), "count")]
    #[case::padded_name(e(1, " A", Section::Main), "misread")]
    #[case::newline_in_name(e(1, "A\nB", Section::Main), "control")]
    #[case::paren_set_in_name(e(1, "Hazmat Suit (Used)", Section::Main), "misread")]
    fn entries_that_would_be_misread_are_refused(#[case] entry: DeckEntry, #[case] fragment: &str) {
        for format in [ExportFormat::Arena, ExportFormat::Mtgo] {
            let error = export_as(
                format,
                &deck(vec![e(1, "Fine", Section::Main), entry.clone()]),
            )
            .unwrap_err();
            assert!(
                matches!(error, ExportError::InvalidEntry { index: 1, .. }),
                "{format:?}: {error}"
            );
            assert!(error.to_string().contains(fragment), "{error}");
        }
    }

    #[test]
    fn decks_with_line_errors_are_refused() {
        let mut broken = deck(vec![e(1, "A", Section::Main)]);
        broken.errors.push(LineError {
            line: 2,
            message: "bad".into(),
        });
        for format in [ExportFormat::Text, ExportFormat::Arena, ExportFormat::Mtgo] {
            assert_eq!(
                export_as(format, &broken),
                Err(ExportError::HasLineErrors { count: 1 })
            );
        }
    }

    fn section() -> impl Strategy<Value = Section> {
        prop_oneof![
            Just(Section::Main),
            Just(Section::Sideboard),
            Just(Section::Commander),
            Just(Section::Companion),
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
        fn arena_reads_back_as_the_deck_in_arena_order(entries in prop::collection::vec(entry(), 1..25)) {
            let text = export_as(ExportFormat::Arena, &deck(entries.clone())).unwrap();
            let again = parse_as(DeckFormat::Arena, &text);
            prop_assert!(again.errors.is_empty());
            let order = [Section::Commander, Section::Companion, Section::Main, Section::Sideboard];
            let expected: Vec<DeckEntry> = order
                .iter()
                .flat_map(|s| entries.iter().filter(move |e| e.section == *s))
                .cloned()
                .collect();
            prop_assert_eq!(again.entries, expected);
        }

        #[test]
        fn mtgo_never_returns_text_that_reads_back_as_another_deck(entries in prop::collection::vec(entry(), 1..25)) {
            // Either the text reads back as the deck MTGO can hold (checked inside) or it is refused.
            if let Ok(text) = export_as(ExportFormat::Mtgo, &deck(entries.clone())) {
                let again = parse_as(DeckFormat::MtgoText, &text);
                prop_assert!(again.errors.is_empty());
                let total: u32 = entries.iter().map(|e| e.count).sum();
                prop_assert_eq!(again.entries.iter().map(|e| e.count).sum::<u32>(), total);
                prop_assert!(again.entries.iter().all(|e| e.set.is_none() && e.collector_number.is_none()));
                prop_assert!(again.entries.iter().all(|e| e.section != Section::Companion));
            }
        }
    }
}
