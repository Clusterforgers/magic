//! Goal: text parsing assigns every entry to the right section and reports bad lines without losing good ones, whatever the line endings.
use super::super::dialect::text_options;
use super::super::types::DeckFormat;
use super::*;
use proptest::prelude::*;
use rstest::rstest;

pub(super) fn options(format: DeckFormat) -> TextOptions {
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

pub(super) fn sections(deck: &ParsedDeck) -> Vec<(u32, &str, Section)> {
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
fn assigns_sections_in_plain_text(#[case] input: &str, #[case] expected: &[(u32, &str, Section)]) {
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
fn arena_deck_names_with_emoji_and_brackets_stay_in_the_preamble() {
    let input = "About\nName [Cabal] A Faster K'rrik Storm \u{1F608}\u{26A1}\u{FE0F} (cEDH w/ Primer)\n\nCommander\n1 K'rrik, Son of Yawgmoth\n\nDeck\n1 Sol Ring\n";
    let deck = parse_text(input, arena());
    assert!(deck.errors.is_empty(), "{:?}", deck.errors);
    assert_eq!(
        sections(&deck),
        [
            (1, "K'rrik, Son of Yawgmoth", Commander),
            (1, "Sol Ring", Main)
        ]
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
    fn mtgo_text_reads_main_sideboard_stickers_and_the_trailing_commander_block(
        main in 1usize..6,
        side in 0usize..4,
        stickers in 0usize..3,
        commanders in 1usize..4,
    ) {
        let block = |name: &str, n: usize| {
            (0..n).map(|i| format!("1 {name}{i}")).collect::<Vec<_>>().join("\n")
        };
        let mut parts = vec![block("Main", main)];
        if side > 0 {
            parts.push(format!("SIDEBOARD:\n{}", block("Side", side)));
        }
        if stickers > 0 {
            parts.push(format!("STICKERS:\n{}", block("Sticker", stickers)));
        }
        parts.push(block("Commander", commanders));
        let deck = parse_text(&parts.join("\n\n"), mtgo_text());
        let count = |n: usize| u64::try_from(n).unwrap();
        prop_assert!(deck.errors.is_empty());
        prop_assert_eq!(deck.card_count(Main), count(main));
        prop_assert_eq!(deck.card_count(Sideboard), count(side));
        prop_assert_eq!(deck.card_count(Commander), count(commanders));
        prop_assert_eq!(deck.ignored.iter().map(|s| s.count).sum::<u64>(), count(stickers));
    }

    #[test]
    fn arbitrary_text_never_panics_and_accounts_for_every_line(input in "\\PC{0,200}|[0-9a-zA-Z \\n\\r:()\\[\\]*/#,'-]{0,200}") {
        for options in [plain(), arena(), archidekt(), mtgo_text(), options(DeckFormat::Moxfield)] {
            let deck = parse_text(&input, options);
            let content_lines = numbered_lines(input.trim_start_matches('\u{feff}'))
                .filter(|(_, l)| !l.trim().is_empty())
                .count();
            prop_assert!(deck.entries.len() + deck.errors.len() <= content_lines);
        }
    }
}
