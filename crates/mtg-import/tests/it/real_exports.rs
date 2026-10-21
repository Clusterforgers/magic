//! Goal: the deck-list parsers read real Moxfield, Archidekt, Arena and MTGO exports of two Commander decks exactly as each site lays them out, and the one lossy export (Arena) stays lossy on purpose.
//!
//! The fixtures in `tests/fixtures/real/` are verbatim site exports (blank lines, emoji, odd trailing newlines): never reformat them.
//! The Arena and MTGO files come from Moxfield's export menu, so they describe the same list as the Moxfield file; Archidekt's lists were edited separately.
//! Moxfield writes its commander as the unmarked first line, so `parse` leaves it in Main and only `promote_implied_commander` moves it.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::{BTreeMap, BTreeSet};
use std::fs::File;
use std::io::BufReader;
use std::path::PathBuf;

use flate2::read::GzDecoder;
use mtg_import::decklist::{
    DeckEntry, DeckFormat, ParsedDeck, Section, detect, export, parse, promote_implied_commander,
};
use mtg_import::resolve::{CardIndex, resolve_deck};
use mtg_import::scryfall::{CardRecord, cards_jsonl};
use rstest::rstest;

use DeckFormat::{Archidekt, Arena, Moxfield, MtgoText};
use Section::{Commander, Main, Sideboard};

const MOXFIELD_NAJEELA: &str = include_str!("../fixtures/real/moxfield/warrior-queen.txt");
const MOXFIELD_TYRANIDS: &str = include_str!("../fixtures/real/moxfield/tyranids.txt");
const ARCHIDEKT_NAJEELA: &str = include_str!("../fixtures/real/archidekt/warrior-queen.txt");
const ARCHIDEKT_TYRANIDS: &str = include_str!("../fixtures/real/archidekt/tyranids.txt");
const ARENA_NAJEELA: &str = include_str!("../fixtures/real/arena/warrior-queen.txt");
const ARENA_TYRANIDS: &str = include_str!("../fixtures/real/arena/tyranids.txt");
const MTGO_NAJEELA: &str = include_str!("../fixtures/real/mtgo/warrior-queen.txt");
const MTGO_TYRANIDS: &str = include_str!("../fixtures/real/mtgo/tyranids.txt");

const NAJEELA: &str = "Najeela, the Blade-Blossom";
const SWARMLORD: &str = "The Swarmlord";

fn entry(count: u32, name: &str, section: Section, printing: Option<(&str, &str)>) -> DeckEntry {
    let entry = DeckEntry::new(count, name, section);
    match printing {
        Some((set, number)) => entry.with_printing(set, Some(number)),
        None => entry,
    }
}

fn names(deck: &ParsedDeck, section: Section) -> Vec<&str> {
    deck.entries
        .iter()
        .filter(|e| e.section == section)
        .map(|e| e.name.as_str())
        .collect()
}

/// The deck a Commander import ends up with: parsed under the detected format, then promoted.
fn imported(input: &str) -> ParsedDeck {
    promote_implied_commander(detect(input), parse(input))
}

/// Copies per (section, card name), ignoring printings and line order.
fn totals(deck: &ParsedDeck) -> BTreeMap<(Section, String), u32> {
    let mut totals = BTreeMap::new();
    for e in &deck.entries {
        *totals.entry((e.section, e.name.clone())).or_insert(0) += e.count;
    }
    totals
}

#[rstest]
#[case::moxfield_najeela(MOXFIELD_NAJEELA, Moxfield, (100, 9, 0), None, Some(NAJEELA))]
#[case::moxfield_tyranids(MOXFIELD_TYRANIDS, Moxfield, (100, 31, 0), None, Some(SWARMLORD))]
#[case::archidekt_najeela(ARCHIDEKT_NAJEELA, Archidekt, (99, 0, 1), Some(NAJEELA), Some(NAJEELA))]
#[case::archidekt_tyranids(ARCHIDEKT_TYRANIDS, Archidekt, (96, 0, 1), Some(SWARMLORD), Some(SWARMLORD))]
#[case::arena_najeela(ARENA_NAJEELA, Arena, (69, 5, 1), Some(NAJEELA), Some(NAJEELA))]
// Arena's export drops cards that are not on Arena, and the Swarmlord with them.
#[case::arena_tyranids(ARENA_TYRANIDS, Arena, (54, 18, 0), None, None)]
#[case::mtgo_najeela(MTGO_NAJEELA, MtgoText, (99, 9, 1), Some(NAJEELA), Some(NAJEELA))]
#[case::mtgo_tyranids(MTGO_TYRANIDS, MtgoText, (99, 31, 1), Some(SWARMLORD), Some(SWARMLORD))]
fn every_site_layout_is_detected_and_split_into_the_right_sections(
    #[case] input: &str,
    #[case] format: DeckFormat,
    #[case] main_side_commander: (u64, u64, u64),
    #[case] marked_commander: Option<&str>,
    #[case] imported_commander: Option<&str>,
) {
    assert_eq!(detect(input), format);
    let deck = parse(input);
    assert_eq!(deck.errors, Vec::new());
    let (main, side, commander) = main_side_commander;
    assert_eq!(deck.card_count(Main), main);
    assert_eq!(deck.card_count(Sideboard), side);
    assert_eq!(deck.card_count(Commander), commander);
    let listed: u64 = deck.entries.iter().map(|e| u64::from(e.count)).sum();
    assert_eq!(
        listed,
        main + side + commander,
        "nothing may land in Companion or Maybeboard"
    );
    assert_eq!(
        names(&deck, Commander),
        marked_commander.into_iter().collect::<Vec<_>>()
    );
    assert_eq!(
        names(&imported(input), Commander),
        imported_commander.into_iter().collect::<Vec<_>>()
    );
}

#[rstest]
#[case::moxfield_commander_stays_in_main(MOXFIELD_NAJEELA, entry(1, NAJEELA, Main, Some(("BBD", "62"))))]
#[case::moxfield_star_number(MOXFIELD_NAJEELA, entry(1, "Arcane Signet", Main, Some(("SLD", "1492★"))))]
#[case::moxfield_star_number_in_a_digit_set(MOXFIELD_NAJEELA, entry(1, "Final Fortune", Main, Some(("7ED", "182★"))))]
#[case::moxfield_dashed_number(MOXFIELD_NAJEELA, entry(1, "Culling the Weak", Main, Some(("PMEI", "2023-8"))))]
#[case::moxfield_five_character_set(MOXFIELD_NAJEELA, entry(1, "Enlightened Tutor", Main, Some(("PAL00", "6"))))]
#[case::moxfield_without_foil_flag(MOXFIELD_NAJEELA, entry(1, "Badlands", Main, Some(("3ED", "282"))))]
#[case::moxfield_sideboard(MOXFIELD_NAJEELA, entry(1, "Gaea's Cradle", Sideboard, Some(("USG", "321"))))]
#[case::moxfield_digit_led_set(MOXFIELD_TYRANIDS, entry(1, SWARMLORD, Main, Some(("40K", "4"))))]
#[case::moxfield_count_seven(MOXFIELD_TYRANIDS, entry(7, "Forest", Main, Some(("ELD", "267"))))]
#[case::moxfield_same_card_in_the_sideboard(MOXFIELD_TYRANIDS, entry(1, "Forest", Sideboard, Some(("ELD", "267"))))]
#[case::moxfield_letter_number(MOXFIELD_TYRANIDS, entry(1, "Temur Ascendancy", Main, Some(("PKTK", "207p"))))]
#[case::moxfield_prefixed_number(MOXFIELD_TYRANIDS, entry(1, "Triumph of the Hordes", Main, Some(("PLST", "NPH-123"))))]
#[case::moxfield_sideboard_count(MOXFIELD_TYRANIDS, entry(3, "Mountain", Sideboard, Some(("ELD", "262"))))]
#[case::archidekt_letter_number(ARCHIDEKT_NAJEELA, entry(1, "Assassin's Trophy", Main, Some(("PMKM", "187p"))))]
#[case::archidekt_commander_with_etched_flag_and_tags(ARCHIDEKT_NAJEELA, entry(1, NAJEELA, Commander, Some(("CMR", "514"))))]
#[case::archidekt_star_number_foil_category_tags(ARCHIDEKT_NAJEELA, entry(1, "Reconnaissance", Main, Some(("SLD", "1575★"))))]
#[case::archidekt_etched_flag_then_category(ARCHIDEKT_NAJEELA, entry(1, "General Marhault Elsdragon", Main, Some(("DMC", "53"))))]
#[case::archidekt_commander_in_a_card_name(ARCHIDEKT_NAJEELA, entry(1, "Rushblade Commander", Main, Some(("BBD", "77"))))]
#[case::archidekt_commander_tag(ARCHIDEKT_TYRANIDS, entry(1, SWARMLORD, Commander, Some(("40K", "321"))))]
#[case::archidekt_count_sixteen(ARCHIDEKT_TYRANIDS, entry(16, "Forest", Main, Some(("40K", "317"))))]
#[case::archidekt_count_two(ARCHIDEKT_TYRANIDS, entry(2, "Island", Main, Some(("40K", "309"))))]
#[case::arena_commander_has_no_printing(ARENA_NAJEELA, entry(1, NAJEELA, Commander, None))]
#[case::arena_last_line_without_newline(
    ARENA_NAJEELA,
    entry(1, "Professional Face-Breaker", Sideboard, None)
)]
#[case::arena_count_seven(ARENA_TYRANIDS, entry(7, "Forest", Main, None))]
#[case::arena_sideboard_count(ARENA_TYRANIDS, entry(2, "Island", Sideboard, None))]
#[case::mtgo_trailing_commander(MTGO_NAJEELA, entry(1, NAJEELA, Commander, None))]
#[case::mtgo_sideboard(MTGO_NAJEELA, entry(1, "Gaea's Cradle", Sideboard, None))]
#[case::mtgo_trailing_commander_after_a_long_sideboard(
    MTGO_TYRANIDS,
    entry(1, SWARMLORD, Commander, None)
)]
#[case::mtgo_count_five(MTGO_TYRANIDS, entry(5, "Island", Main, None))]
fn real_entries_keep_name_printing_and_section_exactly(
    #[case] input: &str,
    #[case] expected: DeckEntry,
) {
    assert!(
        parse(input).entries.contains(&expected),
        "missing {expected:?}"
    );
}

#[rstest]
#[case::moxfield_najeela(MOXFIELD_NAJEELA)]
#[case::moxfield_tyranids(MOXFIELD_TYRANIDS)]
#[case::archidekt_najeela(ARCHIDEKT_NAJEELA)]
#[case::archidekt_tyranids(ARCHIDEKT_TYRANIDS)]
#[case::arena_najeela(ARENA_NAJEELA)]
#[case::arena_tyranids(ARENA_TYRANIDS)]
#[case::mtgo_najeela(MTGO_NAJEELA)]
#[case::mtgo_tyranids(MTGO_TYRANIDS)]
fn flags_categories_and_tags_never_leak_into_names_or_printings(#[case] input: &str) {
    for e in parse(input).entries {
        assert!(!e.name.is_empty() && e.name == e.name.trim(), "{e:?}");
        assert!(
            !e.name.contains(['*', '[', ']', '^', '(', ')', '{', '}']),
            "{e:?}"
        );
        let set_is_code = e.set.as_deref().is_none_or(|set| {
            (2..=6).contains(&set.len())
                && set
                    .bytes()
                    .all(|b| b.is_ascii_digit() || b.is_ascii_uppercase())
        });
        let number_is_bare = e.collector_number.as_deref().is_none_or(|number| {
            !number.is_empty() && !number.contains([' ', '*', '[', '^', '(']) && e.set.is_some()
        });
        assert!(set_is_code && number_is_bare, "{e:?}");
    }
}

#[rstest]
#[case::najeela(MOXFIELD_NAJEELA, MTGO_NAJEELA)]
#[case::tyranids(MOXFIELD_TYRANIDS, MTGO_TYRANIDS)]
fn mtgo_text_and_moxfield_describe_the_same_deck_once_the_commander_is_promoted(
    #[case] moxfield: &str,
    #[case] mtgo: &str,
) {
    assert_eq!(totals(&imported(moxfield)), totals(&imported(mtgo)));
}

/// Arena's export silently drops cards that are not on Arena; the parser must report what the file holds, not what the deck had.
#[rstest]
#[case::najeela(MOXFIELD_NAJEELA, ARENA_NAJEELA, 109, 75)]
#[case::tyranids(MOXFIELD_TYRANIDS, ARENA_TYRANIDS, 131, 72)]
fn arena_export_is_a_strict_subset_of_the_deck_it_came_from(
    #[case] moxfield: &str,
    #[case] arena: &str,
    #[case] moxfield_cards: u32,
    #[case] arena_cards: u32,
) {
    let full = totals(&imported(moxfield));
    let lossy = totals(&imported(arena));
    for (key, copies) in &lossy {
        assert!(
            full.get(key).is_some_and(|n| n >= copies),
            "Arena invented {key:?}"
        );
    }
    assert_eq!(full.values().sum::<u32>(), moxfield_cards);
    assert_eq!(lossy.values().sum::<u32>(), arena_cards);
}

#[rstest]
#[case::najeela(MOXFIELD_NAJEELA, NAJEELA, (99, 9, 1))]
#[case::tyranids(MOXFIELD_TYRANIDS, SWARMLORD, (99, 31, 1))]
fn moxfield_commander_is_the_unmarked_first_line(
    #[case] input: &str,
    #[case] commander: &str,
    #[case] main_side_commander: (u64, u64, u64),
) {
    let parsed = parse(input);
    let promoted = promote_implied_commander(detect(input), parsed.clone());
    assert_eq!(promoted.entries[0].name, commander);
    assert_eq!(promoted.entries[0].section, Commander);
    assert_eq!(promoted.entries[1..], parsed.entries[1..]);
    let (main, side, cmd) = main_side_commander;
    assert_eq!(promoted.card_count(Main), main);
    assert_eq!(promoted.card_count(Sideboard), side);
    assert_eq!(promoted.card_count(Commander), cmd);
}

#[rstest]
#[case::archidekt_najeela(ARCHIDEKT_NAJEELA)]
#[case::archidekt_tyranids(ARCHIDEKT_TYRANIDS)]
#[case::arena_najeela(ARENA_NAJEELA)]
#[case::arena_tyranids_which_lost_its_commander(ARENA_TYRANIDS)]
#[case::mtgo_najeela(MTGO_NAJEELA)]
#[case::mtgo_tyranids(MTGO_TYRANIDS)]
fn commander_import_leaves_every_other_site_alone(#[case] input: &str) {
    let parsed = parse(input);
    assert_eq!(
        promote_implied_commander(detect(input), parsed.clone()),
        parsed
    );
}

#[rstest]
#[case::moxfield_najeela(MOXFIELD_NAJEELA)]
#[case::moxfield_tyranids(MOXFIELD_TYRANIDS)]
#[case::archidekt_najeela(ARCHIDEKT_NAJEELA)]
#[case::archidekt_tyranids(ARCHIDEKT_TYRANIDS)]
#[case::arena_najeela(ARENA_NAJEELA)]
#[case::arena_tyranids(ARENA_TYRANIDS)]
#[case::mtgo_najeela(MTGO_NAJEELA)]
#[case::mtgo_tyranids(MTGO_TYRANIDS)]
fn exporting_a_real_deck_and_parsing_it_back_changes_nothing(#[case] input: &str) {
    let deck = parse(input);
    let text = export(&deck).unwrap();
    assert_eq!(parse(&text), deck);
    assert_eq!(export(&parse(&text)).unwrap(), text);
    let imported = imported(input);
    assert_eq!(parse(&export(&imported).unwrap()), imported);
}

#[rstest]
#[case::moxfield_najeela(MOXFIELD_NAJEELA)]
#[case::moxfield_tyranids(MOXFIELD_TYRANIDS)]
#[case::archidekt_najeela(ARCHIDEKT_NAJEELA)]
#[case::archidekt_tyranids(ARCHIDEKT_TYRANIDS)]
#[case::arena_najeela(ARENA_NAJEELA)]
#[case::arena_tyranids(ARENA_TYRANIDS)]
#[case::mtgo_najeela(MTGO_NAJEELA)]
#[case::mtgo_tyranids(MTGO_TYRANIDS)]
fn windows_line_endings_change_nothing(#[case] input: &str) {
    let crlf = input.replace('\n', "\r\n");
    assert_eq!(detect(&crlf), detect(input));
    assert_eq!(parse(&crlf), parse(input));
}

const ALL: [&str; 8] = [
    MOXFIELD_NAJEELA,
    MOXFIELD_TYRANIDS,
    ARCHIDEKT_NAJEELA,
    ARCHIDEKT_TYRANIDS,
    ARENA_NAJEELA,
    ARENA_TYRANIDS,
    MTGO_NAJEELA,
    MTGO_TYRANIDS,
];

/// Secret Lair printings that Moxfield lists by flavor name (`The Ball` is Fellwar Stone, SLD 1040), which a name index cannot see.
const KNOWN_FLAVOR_NAMES: [&str; 1] = ["The Ball"];

/// Needs the Scryfall oracle dump described in `real_data.rs`.
#[test]
#[ignore = "needs MTG_REAL_DATA_DIR"]
fn real_exports_resolve_against_real_scryfall_data_except_known_flavor_names() {
    let dir = std::env::var("MTG_REAL_DATA_DIR").expect("set MTG_REAL_DATA_DIR");
    let mut dumps: Vec<PathBuf> = std::fs::read_dir(dir)
        .unwrap()
        .map(|file| file.unwrap().path())
        .filter(|path| path.to_string_lossy().contains("/oracle-cards-"))
        .collect();
    dumps.sort();
    let reader = GzDecoder::new(BufReader::new(File::open(dumps.pop().unwrap()).unwrap()));
    let records: Vec<CardRecord> = cards_jsonl(reader).map(Result::unwrap).collect();
    let index = CardIndex::from_records(&records);
    let mut unresolved = BTreeSet::new();
    for input in ALL {
        let resolution = resolve_deck(&index, &parse(input));
        unresolved.extend(resolution.unresolved.into_iter().map(|u| u.entry.name));
    }
    assert_eq!(
        unresolved.into_iter().collect::<Vec<_>>(),
        KNOWN_FLAVOR_NAMES
    );
}
