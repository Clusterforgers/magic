//! Goal: every supported deck-list format parses recorded real-world style exports into the same entries, detection picks the right format, and malformed input degrades into per-line errors.

use mtg_import::decklist::{
    DeckEntry, DeckFormat, ParsedDeck, Section, detect, export, parse, parse_as,
};
use mtg_import::resolve::normalize_name;
use rstest::rstest;

const PLAIN: &str = include_str!("../fixtures/deck_plain.txt");
const PLAIN_BLANK: &str = include_str!("../fixtures/deck_plain_blank_sideboard.txt");
const ARENA: &str = include_str!("../fixtures/deck_arena.txt");
const MOXFIELD: &str = include_str!("../fixtures/deck_moxfield.txt");
const ARCHIDEKT: &str = include_str!("../fixtures/deck_archidekt.txt");
const MTGO: &str = include_str!("../fixtures/deck_mtgo.dek");
const ARENA_COMMANDER: &str = include_str!("../fixtures/deck_arena_commander.txt");
const ARCHIDEKT_COMMANDER: &str = include_str!("../fixtures/deck_archidekt_commander.txt");
const MESSY: &str = include_str!("../fixtures/deck_messy.txt");
const CRLF_BOM: &[u8] = include_bytes!("../fixtures/deck_crlf_bom.txt");
const GARBAGE: &[u8] = include_bytes!("../fixtures/deck_garbage.bin");

/// (count, normalized name, section), the format-independent meaning of an entry.
fn meaning(deck: &ParsedDeck) -> Vec<(u32, String, Section)> {
    deck.entries
        .iter()
        .map(|e| (e.count, normalize_name(&e.name), e.section))
        .collect()
}

fn summary(deck: &ParsedDeck) -> Vec<(u32, &str, Section)> {
    deck.entries
        .iter()
        .map(|e| (e.count, e.name.as_str(), e.section))
        .collect()
}

#[rstest]
#[case::plain(PLAIN, DeckFormat::PlainText)]
#[case::plain_blank(PLAIN_BLANK, DeckFormat::PlainText)]
#[case::arena(ARENA, DeckFormat::Arena)]
#[case::arena_commander(ARENA_COMMANDER, DeckFormat::Arena)]
#[case::moxfield(MOXFIELD, DeckFormat::Moxfield)]
#[case::archidekt(ARCHIDEKT, DeckFormat::Archidekt)]
#[case::archidekt_commander(ARCHIDEKT_COMMANDER, DeckFormat::Archidekt)]
#[case::mtgo(MTGO, DeckFormat::MtgoDek)]
#[case::messy(MESSY, DeckFormat::PlainText)]
fn fixtures_are_detected(#[case] input: &str, #[case] expected: DeckFormat) {
    assert_eq!(detect(input), expected);
}

#[rstest]
#[case::plain(PLAIN)]
#[case::plain_blank(PLAIN_BLANK)]
#[case::arena(ARENA)]
#[case::moxfield(MOXFIELD)]
#[case::archidekt(ARCHIDEKT)]
#[case::mtgo(MTGO)]
fn the_same_deck_in_every_format_means_the_same_thing(#[case] input: &str) {
    let deck = parse(input);
    assert!(deck.errors.is_empty(), "{:?}", deck.errors);
    assert_eq!(
        meaning(&deck),
        vec![
            (4, "lightning bolt".to_owned(), Section::Main),
            (4, "shock".to_owned(), Section::Main),
            (2, "fire // ice".to_owned(), Section::Main),
            (1, "sol ring".to_owned(), Section::Main),
            (20, "mountain".to_owned(), Section::Main),
            (2, "pyroblast".to_owned(), Section::Sideboard),
            (1, "aether vial".to_owned(), Section::Sideboard),
        ]
    );
    assert_eq!(deck.card_count(Section::Main), 31);
    assert_eq!(deck.card_count(Section::Sideboard), 3);
}

#[rstest]
#[case::plain(PLAIN, None)]
#[case::plain_blank(PLAIN_BLANK, Some(("M10", "146")))]
#[case::arena(ARENA, Some(("M10", "146")))]
#[case::moxfield(MOXFIELD, Some(("M10", "146")))]
#[case::archidekt(ARCHIDEKT, Some(("M10", "146")))]
#[case::mtgo(MTGO, None)]
fn printing_hints_are_kept_where_the_format_has_them(
    #[case] input: &str,
    #[case] first: Option<(&str, &str)>,
) {
    let deck = parse(input);
    let first_entry = &deck.entries[0];
    assert_eq!(first_entry.set.as_deref(), first.map(|(s, _)| s));
    assert_eq!(
        first_entry.collector_number.as_deref(),
        first.map(|(_, n)| n)
    );
}

#[test]
fn display_names_are_kept_exactly_as_written() {
    let plain = parse(PLAIN);
    assert_eq!(plain.entries[2].name, "Fire // Ice");
    assert_eq!(plain.entries[6].name, "Æther Vial");
    assert_eq!(parse(MTGO).entries[2].name, "Fire/Ice");
    assert_eq!(parse(ARENA).entries[6].name, "Aether Vial");
}

#[test]
fn arena_commander_export_has_a_command_zone() {
    let deck = parse(ARENA_COMMANDER);
    assert_eq!(deck.errors, Vec::new());
    assert_eq!(
        summary(&deck),
        [
            (1, "Atraxa, Praetors' Voice", Section::Commander),
            (1, "Sol Ring", Section::Main),
            (1, "Arcane Signet", Section::Main),
            (36, "Forest", Section::Main),
        ]
    );
}

#[test]
fn archidekt_categories_pick_the_sections() {
    let deck = parse(ARCHIDEKT_COMMANDER);
    assert_eq!(deck.errors, Vec::new());
    assert_eq!(
        summary(&deck),
        [
            (1, "Atraxa, Praetors' Voice", Section::Commander),
            (1, "Sol Ring", Section::Main),
            (1, "Arcane Signet", Section::Main),
            (36, "Forest", Section::Main),
            (1, "Jace, the Mind Sculptor", Section::Maybeboard),
        ]
    );
}

#[test]
fn messy_list_keeps_good_lines_and_reports_each_bad_one() {
    let deck = parse(MESSY);
    assert_eq!(
        summary(&deck),
        [
            (4, "Lightning Bolt", Section::Main),
            (2, "Atraxa, Praetors' Voice", Section::Main),
            (1, "Pyroblast", Section::Sideboard),
        ]
    );
    let errors: Vec<(usize, &str)> = deck
        .errors
        .iter()
        .map(|e| (e.line, e.message.as_str()))
        .collect();
    assert_eq!(
        errors,
        [
            (3, "expected a card count before the name"),
            (4, "card count must be at least 1"),
            (5, "expected a card count before the name"),
            (7, "card count is too large (maximum 999)"),
            (8, "missing card name"),
        ]
    );
    assert_eq!(
        deck.errors[0].to_string(),
        "line 3: expected a card count before the name"
    );
}

#[test]
fn crlf_and_bom_do_not_change_the_result() {
    let text = std::str::from_utf8(CRLF_BOM).unwrap();
    assert!(text.starts_with('\u{feff}') && text.contains("\r\n"));
    assert_eq!(detect(text), DeckFormat::PlainText);
    let deck = parse(text);
    assert_eq!(deck.errors, Vec::new());
    assert_eq!(
        summary(&deck),
        [
            (4, "Lightning Bolt", Section::Main),
            (4, "Shock", Section::Main),
            (2, "Pyroblast", Section::Sideboard),
        ]
    );
    let lf = text.trim_start_matches('\u{feff}').replace("\r\n", "\n");
    assert_eq!(parse(&lf), deck);
}

#[test]
fn binary_garbage_becomes_line_errors_not_a_panic() {
    let text = String::from_utf8_lossy(GARBAGE);
    for format in [
        DeckFormat::PlainText,
        DeckFormat::Arena,
        DeckFormat::Moxfield,
        DeckFormat::Archidekt,
        DeckFormat::MtgoDek,
    ] {
        let deck = parse_as(format, &text);
        assert!(deck.entries.len() + deck.errors.len() >= 1, "{format:?}");
    }
    let deck = parse(&text);
    assert_eq!(deck.errors.len(), 3, "{:?}", deck.errors);
    assert_eq!(deck.entries.len(), 1);
    assert!(deck.entries[0].name.contains('\u{fffd}'));
}

#[rstest]
#[case::plain(PLAIN)]
#[case::plain_blank(PLAIN_BLANK)]
#[case::arena(ARENA)]
#[case::arena_commander(ARENA_COMMANDER)]
#[case::moxfield(MOXFIELD)]
#[case::archidekt(ARCHIDEKT)]
#[case::archidekt_commander(ARCHIDEKT_COMMANDER)]
#[case::mtgo(MTGO)]
fn exporting_any_fixture_and_parsing_it_back_changes_nothing(#[case] input: &str) {
    let deck = parse(input);
    let text = export(&deck).unwrap();
    assert_eq!(parse(&text), deck);
    assert_eq!(export(&parse(&text)).unwrap(), text);
}

#[test]
fn hostile_inputs_are_contained() {
    let cases: Vec<(&str, String)> = vec![
        (
            "huge count",
            "99999999999999999999999999 Lightning Bolt".into(),
        ),
        ("huge name", format!("1 {}", "x".repeat(2_000_000))),
        ("many lines", "1 Island\n".repeat(200_000)),
        ("only newlines", "\n".repeat(100_000)),
        ("deep parens", format!("1 {}", "(".repeat(100_000))),
        ("many markers", format!("1 Island{}", " *F*".repeat(50_000))),
        (
            "many categories",
            format!("1x Island (c16) 1{}", " [a]".repeat(50_000)),
        ),
        ("nul bytes", "\0\0\0\n1 Island\0\n".into()),
        ("lone surrogates replaced", "1 \u{fffd}\u{fffd}\n".into()),
        ("zero width joiners", "1 Is\u{200d}land\n".into()),
        ("right to left", "1 \u{202e}dnaslI\n".into()),
        ("emoji", "4 \u{1f525} Bolt \u{1f525}\n".into()),
        ("only comments", "// a\n# b\n// c\n".repeat(1000)),
        ("xml bomb", "<Deck>".repeat(10_000)),
    ];
    for (label, input) in cases {
        let deck = parse(&input);
        let lines = input.lines().count();
        assert!(
            deck.entries.len() + deck.errors.len() <= lines.max(1),
            "{label}"
        );
    }
}

#[test]
fn entries_can_be_built_by_hand_and_exported() {
    let deck = ParsedDeck {
        entries: vec![
            DeckEntry::new(1, "Atraxa, Praetors' Voice", Section::Commander)
                .with_printing("c16", Some("28")),
            DeckEntry::new(99, "Forest", Section::Main),
        ],
        errors: vec![],
        ..ParsedDeck::default()
    };
    assert_eq!(
        export(&deck).unwrap(),
        "Commander\n1 Atraxa, Praetors' Voice (C16) 28\n\nDeck\n99 Forest\n"
    );
}
