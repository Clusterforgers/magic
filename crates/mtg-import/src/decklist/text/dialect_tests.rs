//! Goal: the dialect rules the corpus exposed hold for every notation: Archidekt's first category decides the board and its headers never become cards, sections that are no deck cards are counted and kept out, and Moxfield tags never reach names or printings.
use super::tests::{options, sections};
use super::*;
use proptest::prelude::*;
use rstest::rstest;

use super::super::types::DeckFormat;
use Section::{Commander, Main, Maybeboard, Sideboard};

fn archidekt() -> TextOptions {
    options(DeckFormat::Archidekt)
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
#[case::excluded_custom_category("1x A (abc) 1 [Last Minute Cuts{noDeck}{noPrice}]", Maybeboard)]
#[case::maybeboard("1x A (abc) 1 [Maybeboard{noDeck}{noPrice},Creature]", Maybeboard)]
#[case::tag_label_with_spaces_and_commas("1x A (abc) 1 [Ramp] ^Don't Have, really,#f47373^", Main)]
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

proptest! {
    #[test]
    fn a_sticker_section_takes_every_card_after_it_and_no_others(
        copies in prop::collection::vec(1u32..=4, 1..12),
        cut in 0usize..12,
    ) {
        let cut = cut.min(copies.len());
        let mut lines: Vec<String> = copies
            .iter()
            .enumerate()
            .map(|(i, n)| format!("{n} Card {i} (SET) {i}"))
            .collect();
        lines.insert(cut, "STICKERS:".to_owned());
        let deck = parse_text(&lines.join("\n"), options(DeckFormat::Moxfield));
        let kept: u64 = copies[..cut].iter().map(|&n| u64::from(n)).sum();
        let left_out: u64 = copies[cut..].iter().map(|&n| u64::from(n)).sum();
        prop_assert!(deck.errors.is_empty());
        prop_assert_eq!(deck.card_count(Main), kept);
        prop_assert_eq!(deck.ignored.iter().map(|s| s.count).sum::<u64>(), left_out);
    }
}
