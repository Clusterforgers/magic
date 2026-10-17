//! Line-oriented text deck parsing shared by the plain, Arena, MTGO text, Moxfield and Archidekt dialects.

use super::blocks::{BlankLines, Blocks, Place};
use super::line::{EntryError, LineOptions, parse_entry};
use super::section::{Header, header, section_from_category, strip_sideboard_prefix};
use super::types::{DeckEntry, LineError, ParsedDeck, Section};

/// Per-dialect switches for text parsing.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TextOptions {
    /// Entry-line grammar switches.
    pub line: LineOptions,
    /// What a blank line means.
    pub blank_lines: BlankLines,
    /// Whether Arena's `About` / `Name` preamble is skipped.
    pub about_block: bool,
    /// Whether a line that cannot be an entry, alone above a block of entries, is a category or card-type header (Archidekt).
    pub category_headers: bool,
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
    Header(Header),
    Label,
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
    match header(text) {
        Some(header) => Line::Header(header),
        None => Line::Entry(text),
    }
}

/// Finds the category and card-type headers: lines no entry could be, standing alone above a block of entries.
fn mark_labels(lines: &mut [(usize, Line<'_>)], options: LineOptions) {
    for at in 0..lines.len() {
        let Line::Entry(text) = lines[at].1 else {
            continue;
        };
        let after_gap = at == 0 || matches!(lines[at - 1].1, Line::Blank);
        let above_entries = matches!(lines.get(at + 1), Some((_, Line::Entry(_))));
        if after_gap
            && above_entries
            && matches!(
                parse_entry(text, options),
                Err(EntryError::MissingCount | EntryError::NoSpaceAfterCount)
            )
        {
            lines[at].1 = Line::Label;
        }
    }
}

fn parse_entry_line(
    text: &str,
    number: usize,
    options: TextOptions,
    blocks: &mut Blocks,
    deck: &mut ParsedDeck,
) {
    let (text, prefixed) = match strip_sideboard_prefix(text) {
        Some(rest) => (rest, Some(Section::Sideboard)),
        None => (text, None),
    };
    match parse_entry(text, options.line) {
        Ok(raw) => match blocks.place {
            Place::Ignored(at) => {
                if let Some(ignored) = deck.ignored.get_mut(at) {
                    ignored.count += u64::from(raw.count);
                }
                blocks.counted();
            }
            Place::In(current) => {
                let from_category = raw.category.as_deref().and_then(section_from_category);
                let explicit = prefixed.or(from_category);
                if explicit.is_none() {
                    blocks.counted();
                }
                deck.entries.push(DeckEntry {
                    count: raw.count,
                    name: raw.name,
                    set: raw.set,
                    collector_number: raw.collector_number,
                    section: explicit.unwrap_or(current),
                });
            }
        },
        Err(error) => deck.errors.push(LineError {
            line: number,
            message: error.to_string(),
        }),
    }
}

/// Parses a text deck list under `options`; bad lines become [`LineError`]s.
pub fn parse_text(input: &str, options: TextOptions) -> ParsedDeck {
    let mut in_preamble = false;
    let mut lines: Vec<(usize, Line<'_>)> = numbered_lines(input.trim_start_matches('\u{feff}'))
        .map(|(number, raw)| (number, classify(raw, &mut in_preamble, options)))
        .collect();
    if options.category_headers {
        mark_labels(&mut lines, options.line);
    }
    let mut blocks = Blocks::new(
        lines
            .iter()
            .any(|(_, l)| matches!(l, Line::Header(_) | Line::Label)),
        options.blank_lines,
    );

    let mut deck = ParsedDeck::default();
    for (number, line) in lines {
        match line {
            Line::Blank => blocks.on_blank(),
            Line::Comment | Line::Preamble => {}
            Line::Header(header) => blocks.on_header(header, &mut deck),
            Line::Label => blocks.on_label(),
            Line::Entry(text) => parse_entry_line(text, number, options, &mut blocks, &mut deck),
        }
    }
    deck.ignored.retain(|section| section.count > 0);
    deck
}

#[cfg(test)]
mod tests {
    //! Goal: text parsing assigns every entry to the right section and reports bad lines without losing good ones, whatever the line endings.
    use super::super::dialect::text_options;
    use super::super::types::DeckFormat;
    use super::*;
    use proptest::prelude::*;
    use rstest::rstest;

    fn options(format: DeckFormat) -> TextOptions {
        text_options(format).unwrap()
    }

    fn plain() -> TextOptions {
        options(DeckFormat::PlainText)
    }

    fn arena() -> TextOptions {
        options(DeckFormat::Arena)
    }

    fn archidekt() -> TextOptions {
        options(DeckFormat::Archidekt)
    }

    fn mtgo_text() -> TextOptions {
        options(DeckFormat::MtgoText)
    }

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
        let deck = parse_text(input, plain());
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
    #[case::no_sideboard_the_block_after_the_main_deck("1 A\n\n1 B", &[(1, "A", Main), (1, "B", Commander)])]
    #[case::no_sideboard_partners("1 A\n1 A2\n\n1 B\n1 C", &[(1, "A", Main), (1, "A2", Main), (1, "B", Commander), (1, "C", Commander)])]
    #[case::no_blank_no_command_zone("1 A\n1 B", &[(1, "A", Main), (1, "B", Main)])]
    #[case::sb_prefix_lines_are_no_block("1 A\nSB: 1 B\n\n1 C", &[(1, "A", Main), (1, "B", Sideboard), (1, "C", Commander)])]
    #[case::stickers_then_commander("1 A\n\nSTICKERS:\n1 S\n\n1 C", &[(1, "A", Main), (1, "C", Commander)])]
    #[case::sideboard_stickers_then_commander("1 A\n\nSIDEBOARD:\n1 B\n\nSTICKERS:\n1 S\n2 T\n\n1 C\n1 D", &[(1, "A", Main), (1, "B", Sideboard), (1, "C", Commander), (1, "D", Commander)])]
    fn mtgo_text_lists_the_commander_after_the_sideboard(
        #[case] input: &str,
        #[case] expected: &[(u32, &str, Section)],
    ) {
        let deck = parse_text(input, mtgo_text());
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
        let deck = parse_text(input, plain());
        assert!(deck.errors.is_empty(), "{:?}", deck.errors);
        assert_eq!(
            sections(&deck),
            [(4, "Bolt", Main), (2, "Shock", Sideboard)]
        );
    }

    #[test]
    fn arena_export_with_about_block() {
        let input = "About\nName Mono Red\n\nDeck\n4 Lightning Bolt (M10) 146\n\nSideboard\n2 Pyroblast (ICE) 213\n";
        let deck = parse_text(input, arena());
        assert!(deck.errors.is_empty(), "{:?}", deck.errors);
        assert_eq!(
            sections(&deck),
            [(4, "Lightning Bolt", Main), (2, "Pyroblast", Sideboard)]
        );
        assert_eq!(deck.entries[0].set.as_deref(), Some("M10"));
    }

    #[rstest]
    #[case::top_flag("1x A (abc) 1 [Commander{top}]", Commander)]
    #[case::top_flag_on_a_custom_category("1x A (abc) 1 [Partners{top},Ramp]", Commander)]
    #[case::plain_category("1x A (abc) 1 [Ramp]", Main)]
    #[case::first_category_wins("1x A (abc) 1 [Ramp,Maybeboard{noDeck}{noPrice}]", Main)]
    #[case::a_later_top_flag_changes_nothing("1x A (abc) 1 [Ramp,Commander{top}]", Main)]
    #[case::sideboard("1x A (abc) 1 [Sideboard,Instant]", Sideboard)]
    #[case::sideboard_even_when_excluded("1x A (abc) 1 [Sideboard{noDeck}{noPrice}]", Sideboard)]
    #[case::sideboard_before_a_top_flag("1x A (abc) 1 [Sideboard,Commander{top}]", Sideboard)]
    #[case::sideboard_in_a_longer_name_is_just_excluded(
        "1x A (abc) 1 [1v1 Sideboard{noDeck}{noPrice},Creature]",
        Maybeboard
    )]
    #[case::excluded_custom_category(
        "1x A (abc) 1 [Last Minute Cuts{noDeck}{noPrice}]",
        Maybeboard
    )]
    #[case::maybeboard("1x A (abc) 1 [Maybeboard{noDeck}{noPrice},Creature]", Maybeboard)]
    #[case::tag_label_with_spaces_and_commas(
        "1x A (abc) 1 [Ramp] ^Don't Have, really,#f47373^",
        Main
    )]
    #[case::flag_category_tag("1x A (abc) 1 *E* [Maybeboard{noDeck}] ^Have,#37d67a^", Maybeboard)]
    fn archidekt_decides_the_board_by_the_first_category(
        #[case] line: &str,
        #[case] expected: Section,
    ) {
        let deck = parse_text(line, archidekt());
        assert!(deck.errors.is_empty(), "{:?}", deck.errors);
        assert_eq!(sections(&deck), [(1, "A", expected)]);
        assert_eq!(deck.entries[0].set.as_deref(), Some("ABC"));
        assert_eq!(deck.entries[0].collector_number.as_deref(), Some("1"));
    }

    #[rstest]
    #[case::generic_roles(
        "Commander\n1x A\n\nMainboard\n1x B\n\nSideboard\n1x C\n\nMaybeboard\n1x D",
        &[(1, "A", Commander), (1, "B", Main), (1, "C", Sideboard), (1, "D", Maybeboard)]
    )]
    #[case::category_headers_leave_the_board_to_the_brackets(
        "Commander\n1x A [Commander{top}]\n\nBig Stick\n1x B [Big Stick]\n\nLast Cuts\n1x C [Last Cuts{noDeck}]",
        &[(1, "A", Commander), (1, "B", Main), (1, "C", Maybeboard)]
    )]
    #[case::card_types("Land\n1x A [Land]\n\nCreature\n1x B [Ramp,Creature]", &[(1, "A", Main), (1, "B", Main)])]
    #[case::numbers_and_emoji_in_headers(
        "5: Lands\n1x A\n\n1v1 Sideboard\n1x B\n\n🟢 Main Deck\n1x C\n\n- Removed 2019 to 2026\n1x D",
        &[(1, "A", Main), (1, "B", Main), (1, "C", Main), (1, "D", Main)]
    )]
    #[case::a_category_header_ends_a_known_section("Sideboard\n1x A\n\nRamp\n1x B", &[(1, "A", Sideboard), (1, "B", Main)])]
    #[case::a_bracket_beats_the_header("Mainboard\n1x A [Sideboard]\n1x B", &[(1, "A", Sideboard), (1, "B", Main)])]
    fn archidekt_reads_category_and_type_headers(
        #[case] input: &str,
        #[case] expected: &[(u32, &str, Section)],
    ) {
        let deck = parse_text(input, archidekt());
        assert!(deck.errors.is_empty(), "{:?}", deck.errors);
        assert_eq!(sections(&deck), expected);
    }

    #[test]
    fn a_stray_line_inside_a_block_is_still_an_error() {
        let deck = parse_text("1x A\nRamp\n1x B\n\nLightning Bolt\n", archidekt());
        assert_eq!(sections(&deck), [(1, "A", Main), (1, "B", Main)]);
        let lines: Vec<usize> = deck.errors.iter().map(|e| e.line).collect();
        assert_eq!(lines, [2, 5]);
    }

    #[rstest]
    #[case::moxfield(
        DeckFormat::Moxfield,
        "1 A (ABC) 1\n\nSIDEBOARD:\n1 B (ABC) 2\n\nSTICKERS:\n1 S (SUNF) 1\n3 T (SUNF) 2\n",
        &[(1, "A", Main), (1, "B", Sideboard)],
        &[("STICKERS", 4)]
    )]
    #[case::mtgo_text(
        DeckFormat::MtgoText,
        "1 A\n\nSTICKERS:\n1 S\n\n1 C",
        &[(1, "A", Main), (1, "C", Commander)],
        &[("STICKERS", 1)]
    )]
    #[case::plain_text_any_case(DeckFormat::PlainText, "4 A\nStickers (2)\n1 S\n1 T", &[(4, "A", Main)], &[("Stickers", 2)])]
    #[case::an_empty_section_is_not_reported(DeckFormat::Moxfield, "1 A (ABC) 1\n\nSTICKERS:\n", &[(1, "A", Main)], &[])]
    #[case::a_recurring_section_is_one_section(
        DeckFormat::Moxfield,
        "1 A (ABC) 1\nSTICKERS:\n1 S\nSIDEBOARD:\n1 B (ABC) 2\nSTICKERS:\n2 T",
        &[(1, "A", Main), (1, "B", Sideboard)],
        &[("STICKERS", 3)]
    )]
    fn sections_that_are_no_deck_cards_are_counted_and_kept_out(
        #[case] format: DeckFormat,
        #[case] input: &str,
        #[case] expected: &[(u32, &str, Section)],
        #[case] ignored: &[(&str, u64)],
    ) {
        let deck = parse_text(input, options(format));
        assert!(deck.errors.is_empty(), "{:?}", deck.errors);
        assert_eq!(sections(&deck), expected);
        let reported: Vec<(&str, u64)> = deck
            .ignored
            .iter()
            .map(|s| (s.name.as_str(), s.count))
            .collect();
        assert_eq!(reported, ignored);
    }

    #[rstest]
    #[case::moxfield_printing_flag_tags(DeckFormat::Moxfield, "1 Ancient Tomb (V12) 1 *F* #!Lands", "Ancient Tomb", Some(("V12", "1")))]
    #[case::moxfield_spaces_and_several_tags(DeckFormat::Moxfield, "3 Beseech the Mirror (WOE) 82 #Card Advantage #Payoff #!Wheel", "Beseech the Mirror", Some(("WOE", "82")))]
    #[case::moxfield_parentheses_in_a_tag(DeckFormat::Moxfield, "1 Sol Ring (BRC) 160 #Mana (LOCKED)", "Sol Ring", Some(("BRC", "160")))]
    #[case::moxfield_custom_card(
        DeckFormat::Moxfield,
        "1 My Homebrew #CustomCard",
        "My Homebrew",
        None
    )]
    #[case::arena_tags_without_printing(
        DeckFormat::Arena,
        "Deck\n1 Ancient Tomb #!Lands",
        "Ancient Tomb",
        None
    )]
    fn hash_tags_never_reach_names_or_printings(
        #[case] format: DeckFormat,
        #[case] input: &str,
        #[case] name: &str,
        #[case] printing: Option<(&str, &str)>,
    ) {
        let deck = parse_text(input, options(format));
        assert!(deck.errors.is_empty(), "{:?}", deck.errors);
        let entry = &deck.entries[0];
        assert_eq!(entry.name, name);
        assert_eq!(
            entry.set.as_deref().zip(entry.collector_number.as_deref()),
            printing
        );
    }

    #[test]
    fn bad_lines_are_reported_with_line_numbers_and_do_not_stop_parsing() {
        let input = "4 Bolt\nShock\n0 Fury\n\n2 Duress\n99999999999 Foo\n3\n";
        let deck = parse_text(input, plain());
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
        let deck = parse_text("4 Bolt\r\n\r\nnope\r\n", plain());
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
            for options in [plain(), arena(), archidekt(), mtgo_text()] {
                let deck = parse_text(&input, options);
                let content_lines = numbered_lines(input.trim_start_matches('\u{feff}'))
                    .filter(|(_, l)| !l.trim().is_empty())
                    .count();
                prop_assert!(deck.entries.len() + deck.errors.len() <= content_lines);
            }
        }
    }
}
