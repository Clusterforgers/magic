//! Line-oriented text deck parsing shared by the plain, Arena, Moxfield and Archidekt dialects.

use super::line::{LineOptions, parse_entry};
use super::section::{header_section, section_from_category, strip_sideboard_prefix};
use super::types::{DeckEntry, LineError, ParsedDeck, Section};

/// Per-dialect switches for text parsing.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TextOptions {
    /// Entry-line grammar switches.
    pub line: LineOptions,
    /// Whether a blank line after the main deck starts the sideboard when no headers exist.
    pub blank_line_sideboard: bool,
    /// Whether Arena's `About` / `Name` preamble is skipped.
    pub about_block: bool,
    /// Whether the headerless block after a sideboard block is the command zone (MTGO text lists commanders last).
    pub commander_after_sideboard: bool,
}

/// Splits on `\n`, `\r\n` and lone `\r`, yielding one-based line numbers.
pub fn numbered_lines(input: &str) -> impl Iterator<Item = (usize, &str)> {
    let mut rest = Some(input);
    let mut number = 0;
    std::iter::from_fn(move || {
        let text = rest?;
        number += 1;
        if let Some(end) = text.find(['\n', '\r']) {
            let skip = if text[end..].starts_with("\r\n") {
                2
            } else {
                1
            };
            rest = Some(&text[end + skip..]);
            Some((number, &text[..end]))
        } else {
            rest = None;
            (!text.is_empty()).then_some((number, text))
        }
    })
}

enum Line<'a> {
    Blank,
    Comment,
    Preamble,
    Header(Section),
    Entry(&'a str),
}

fn classify<'a>(raw: &'a str, in_preamble: &mut bool, options: TextOptions) -> Line<'a> {
    let text = raw.trim();
    if text.is_empty() {
        *in_preamble = false;
        return Line::Blank;
    }
    if text.starts_with("//") || text.starts_with('#') {
        return Line::Comment;
    }
    if *in_preamble {
        return Line::Preamble;
    }
    if options.about_block && text.eq_ignore_ascii_case("about") {
        *in_preamble = true;
        return Line::Preamble;
    }
    match header_section(text) {
        Some(section) => Line::Header(section),
        None => Line::Entry(text),
    }
}

/// Tracks which section the next entry belongs to.
struct Sections {
    current: Section,
    entries_in_block: usize,
    file_has_headers: bool,
    blank_line_sideboard: bool,
    commander_after_sideboard: bool,
}

impl Sections {
    fn on_header(&mut self, section: Section) {
        self.current = section;
        self.entries_in_block = 0;
    }

    /// A blank line ends a command-zone block, the main deck when the file has no headers, or a sideboard that the commander follows.
    fn on_blank(&mut self) {
        if self.entries_in_block == 0 {
            return;
        }
        let next = match self.current {
            Section::Commander | Section::Companion => Section::Main,
            Section::Main if self.blank_line_sideboard && !self.file_has_headers => {
                Section::Sideboard
            }
            Section::Sideboard if self.commander_after_sideboard => Section::Commander,
            _ => return,
        };
        self.current = next;
        self.entries_in_block = 0;
    }
}

fn parse_entry_line(
    text: &str,
    number: usize,
    options: TextOptions,
    sections: &mut Sections,
    deck: &mut ParsedDeck,
) {
    let (text, prefixed) = match strip_sideboard_prefix(text) {
        Some(rest) => (rest, Some(Section::Sideboard)),
        None => (text, None),
    };
    match parse_entry(text, options.line) {
        Ok(raw) => {
            let from_category = raw.category.as_deref().and_then(section_from_category);
            let explicit = prefixed.or(from_category);
            if explicit.is_none() {
                sections.entries_in_block += 1;
            }
            deck.entries.push(DeckEntry {
                count: raw.count,
                name: raw.name,
                set: raw.set,
                collector_number: raw.collector_number,
                section: explicit.unwrap_or(sections.current),
            });
        }
        Err(error) => deck.errors.push(LineError {
            line: number,
            message: error.to_string(),
        }),
    }
}

/// Parses a text deck list under `options`; bad lines become [`LineError`]s.
pub fn parse_text(input: &str, options: TextOptions) -> ParsedDeck {
    let mut in_preamble = false;
    let lines: Vec<(usize, Line<'_>)> = numbered_lines(input.trim_start_matches('\u{feff}'))
        .map(|(number, raw)| (number, classify(raw, &mut in_preamble, options)))
        .collect();
    let mut sections = Sections {
        current: Section::Main,
        entries_in_block: 0,
        file_has_headers: lines.iter().any(|(_, l)| matches!(l, Line::Header(_))),
        blank_line_sideboard: options.blank_line_sideboard,
        commander_after_sideboard: options.commander_after_sideboard,
    };

    let mut deck = ParsedDeck::default();
    for (number, line) in lines {
        match line {
            Line::Blank => sections.on_blank(),
            Line::Comment | Line::Preamble => {}
            Line::Header(section) => sections.on_header(section),
            Line::Entry(text) => parse_entry_line(text, number, options, &mut sections, &mut deck),
        }
    }
    deck
}

#[cfg(test)]
mod tests {
    //! Goal: text parsing assigns every entry to the right section and reports bad lines without losing good ones, whatever the line endings.
    use super::*;
    use proptest::prelude::*;
    use rstest::rstest;

    const PLAIN: TextOptions = TextOptions {
        line: LineOptions {
            bracket_sets: true,
            categories: false,
        },
        blank_line_sideboard: true,
        about_block: false,
        commander_after_sideboard: false,
    };
    const ARENA: TextOptions = TextOptions {
        line: LineOptions {
            bracket_sets: false,
            categories: false,
        },
        blank_line_sideboard: false,
        about_block: true,
        commander_after_sideboard: false,
    };
    const ARCHIDEKT: TextOptions = TextOptions {
        line: LineOptions {
            bracket_sets: false,
            categories: true,
        },
        blank_line_sideboard: false,
        about_block: false,
        commander_after_sideboard: false,
    };
    const MTGO_TEXT: TextOptions = TextOptions {
        line: LineOptions {
            bracket_sets: false,
            categories: false,
        },
        blank_line_sideboard: false,
        about_block: false,
        commander_after_sideboard: true,
    };

    fn sections(deck: &ParsedDeck) -> Vec<(u32, &str, Section)> {
        deck.entries
            .iter()
            .map(|e| (e.count, e.name.as_str(), e.section))
            .collect()
    }

    use Section::{Commander, Companion, Main, Maybeboard, Sideboard};

    #[rstest]
    #[case::all_main("4 Bolt\n2 Shock\n", &[(4, "Bolt", Main), (2, "Shock", Main)])]
    #[case::blank_line_starts_sideboard("4 Bolt\n\n2 Shock\n", &[(4, "Bolt", Main), (2, "Shock", Sideboard)])]
    #[case::several_blank_lines("4 Bolt\n\n\n\n2 Shock", &[(4, "Bolt", Main), (2, "Shock", Sideboard)])]
    #[case::leading_blank_lines_ignored("\n\n4 Bolt\n2 Shock", &[(4, "Bolt", Main), (2, "Shock", Main)])]
    #[case::sb_prefix("4 Bolt\nSB: 2 Shock\n3 Fury", &[(4, "Bolt", Main), (2, "Shock", Sideboard), (3, "Fury", Main)])]
    #[case::sb_prefix_no_space("4 Bolt\nsb:2 Shock", &[(4, "Bolt", Main), (2, "Shock", Sideboard)])]
    #[case::sideboard_header("4 Bolt\nSideboard\n2 Shock", &[(4, "Bolt", Main), (2, "Shock", Sideboard)])]
    #[case::sideboard_header_with_blank("4 Bolt\n\nSideboard\n\n2 Shock", &[(4, "Bolt", Main), (2, "Shock", Sideboard)])]
    #[case::headers_disable_blank_rule("Deck\n4 Bolt\n\n2 Shock\n\nSideboard\n1 Fury", &[(4, "Bolt", Main), (2, "Shock", Main), (1, "Fury", Sideboard)])]
    #[case::commander_then_blank_then_main("Commander\n1 Atraxa\n\n99 Forest", &[(1, "Atraxa", Commander), (99, "Forest", Main)])]
    #[case::commander_then_deck("Commander\n1 Atraxa\n\nDeck\n99 Forest", &[(1, "Atraxa", Commander), (99, "Forest", Main)])]
    #[case::commander_header_blank_first("Commander\n\n1 Atraxa\n\nDeck\n1 Sol Ring", &[(1, "Atraxa", Commander), (1, "Sol Ring", Main)])]
    #[case::companion("Companion\n1 Lurrus\n\nDeck\n4 Bolt\n\nSideboard\n1 Fury", &[(1, "Lurrus", Companion), (4, "Bolt", Main), (1, "Fury", Sideboard)])]
    #[case::maybeboard("4 Bolt\n\nMaybeboard\n1 Fury", &[(4, "Bolt", Main), (1, "Fury", Maybeboard)])]
    #[case::interleaved_sections("Sideboard\n1 A\nDeck\n1 B\nSideboard\n1 C", &[(1, "A", Sideboard), (1, "B", Main), (1, "C", Sideboard)])]
    #[case::comments_ignored("// my deck\n# notes\n4 Bolt\n//2 Shock", &[(4, "Bolt", Main)])]
    #[case::comment_does_not_end_block("4 Bolt\n// c\n2 Shock", &[(4, "Bolt", Main), (2, "Shock", Main)])]
    #[case::colon_headers("4 Bolt\nSIDEBOARD:\n1 Fury", &[(4, "Bolt", Main), (1, "Fury", Sideboard)])]
    #[case::block_after_sideboard_stays_sideboard("4 Bolt\n\nSideboard\n1 Fury\n\n1 Duress", &[(4, "Bolt", Main), (1, "Fury", Sideboard), (1, "Duress", Sideboard)])]
    #[case::empty("", &[])]
    #[case::whitespace_only("  \n\t\n", &[])]
    fn assigns_sections_in_plain_text(
        #[case] input: &str,
        #[case] expected: &[(u32, &str, Section)],
    ) {
        let deck = parse_text(input, PLAIN);
        assert!(deck.errors.is_empty(), "{:?}", deck.errors);
        assert_eq!(sections(&deck), expected);
    }

    #[rstest]
    #[case::trailing_block("1 A\n\nSIDEBOARD:\n1 B\n\n1 C", &[(1, "A", Main), (1, "B", Sideboard), (1, "C", Commander)])]
    #[case::partners_share_the_block("1 A\n\nSIDEBOARD:\n1 B\n\n1 C\n1 D", &[(1, "A", Main), (1, "B", Sideboard), (1, "C", Commander), (1, "D", Commander)])]
    #[case::several_blank_lines("1 A\n\nSIDEBOARD:\n1 B\n\n\n\n1 C\n\n\n", &[(1, "A", Main), (1, "B", Sideboard), (1, "C", Commander)])]
    #[case::crlf("1 A\r\n\r\nSIDEBOARD:\r\n1 B\r\n\r\n1 C\r\n", &[(1, "A", Main), (1, "B", Sideboard), (1, "C", Commander)])]
    #[case::command_zone_ends_at_the_next_blank("1 A\n\nSIDEBOARD:\n1 B\n\n1 C\n\n1 D", &[(1, "A", Main), (1, "B", Sideboard), (1, "C", Commander), (1, "D", Main)])]
    #[case::no_trailing_block("1 A\n\nSIDEBOARD:\n1 B\n\n", &[(1, "A", Main), (1, "B", Sideboard)])]
    #[case::no_sideboard_no_command_zone("1 A\n\n1 B", &[(1, "A", Main), (1, "B", Main)])]
    #[case::sb_prefix_opens_nothing("1 A\nSB: 1 B\n\n1 C", &[(1, "A", Main), (1, "B", Sideboard), (1, "C", Main)])]
    fn mtgo_text_lists_the_commander_after_the_sideboard(
        #[case] input: &str,
        #[case] expected: &[(u32, &str, Section)],
    ) {
        let deck = parse_text(input, MTGO_TEXT);
        assert!(deck.errors.is_empty(), "{:?}", deck.errors);
        assert_eq!(sections(&deck), expected);
    }

    #[rstest]
    #[case::lf("4 Bolt\n\n2 Shock\n")]
    #[case::crlf("4 Bolt\r\n\r\n2 Shock\r\n")]
    #[case::lone_cr("4 Bolt\r\r2 Shock\r")]
    #[case::bom("\u{feff}4 Bolt\n\n2 Shock")]
    #[case::bom_crlf("\u{feff}4 Bolt\r\n\r\n2 Shock")]
    #[case::trailing_spaces("4 Bolt  \n  \n2 Shock  ")]
    #[case::no_trailing_newline("4 Bolt\n\n2 Shock")]
    fn line_ending_variants_parse_identically(#[case] input: &str) {
        let deck = parse_text(input, PLAIN);
        assert!(deck.errors.is_empty(), "{:?}", deck.errors);
        assert_eq!(
            sections(&deck),
            [(4, "Bolt", Main), (2, "Shock", Sideboard)]
        );
    }

    #[test]
    fn arena_export_with_about_block() {
        let input = "About\nName Mono Red\n\nDeck\n4 Lightning Bolt (M10) 146\n\nSideboard\n2 Pyroblast (ICE) 213\n";
        let deck = parse_text(input, ARENA);
        assert!(deck.errors.is_empty(), "{:?}", deck.errors);
        assert_eq!(
            sections(&deck),
            [(4, "Lightning Bolt", Main), (2, "Pyroblast", Sideboard)]
        );
        assert_eq!(deck.entries[0].set.as_deref(), Some("M10"));
    }

    #[test]
    fn archidekt_categories_override_sections() {
        let input = "1x Atraxa (c16) 28 [Commander{top}]\n1x Sol Ring (cmm) 400 [Ramp]\n1x Fury (mh2) 126 [Sideboard]\n1x Foo (abc) 1 [Maybeboard{noDeck}]\n";
        let deck = parse_text(input, ARCHIDEKT);
        assert!(deck.errors.is_empty(), "{:?}", deck.errors);
        assert_eq!(
            sections(&deck),
            [
                (1, "Atraxa", Commander),
                (1, "Sol Ring", Main),
                (1, "Fury", Sideboard),
                (1, "Foo", Maybeboard)
            ]
        );
    }

    #[test]
    fn bad_lines_are_reported_with_line_numbers_and_do_not_stop_parsing() {
        let input = "4 Bolt\nShock\n0 Fury\n\n2 Duress\n99999999999 Foo\n3\n";
        let deck = parse_text(input, PLAIN);
        assert_eq!(
            sections(&deck),
            [(4, "Bolt", Main), (2, "Duress", Sideboard)]
        );
        let lines: Vec<usize> = deck.errors.iter().map(|e| e.line).collect();
        assert_eq!(lines, [2, 3, 6, 7]);
        assert!(deck.errors[0].message.contains("count"));
    }

    #[test]
    fn error_line_numbers_survive_crlf_and_blank_lines() {
        let deck = parse_text("4 Bolt\r\n\r\nnope\r\n", PLAIN);
        assert_eq!(deck.errors.len(), 1);
        assert_eq!(deck.errors[0].line, 3);
    }

    #[rstest]
    #[case::lf("a\nb\nc", &[(1, "a"), (2, "b"), (3, "c")])]
    #[case::crlf("a\r\nb\r\n", &[(1, "a"), (2, "b")])]
    #[case::mixed("a\rb\nc\r\nd", &[(1, "a"), (2, "b"), (3, "c"), (4, "d")])]
    #[case::blank_middle("a\n\nb", &[(1, "a"), (2, ""), (3, "b")])]
    #[case::empty("", &[])]
    #[case::only_newline("\n", &[(1, "")])]
    fn splits_lines_like_every_platform(#[case] input: &str, #[case] expected: &[(usize, &str)]) {
        assert_eq!(numbered_lines(input).collect::<Vec<_>>(), expected);
    }

    proptest! {
        #[test]
        fn arbitrary_text_never_panics_and_accounts_for_every_line(input in "\\PC{0,200}|[0-9a-zA-Z \\n\\r:()\\[\\]*/#,'-]{0,200}") {
            for options in [PLAIN, ARENA, ARCHIDEKT, MTGO_TEXT] {
                let deck = parse_text(&input, options);
                let content_lines = numbered_lines(input.trim_start_matches('\u{feff}'))
                    .filter(|(_, l)| !l.trim().is_empty())
                    .count();
                prop_assert!(deck.entries.len() + deck.errors.len() <= content_lines);
            }
        }
    }
}
