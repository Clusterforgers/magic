//! Goal: every export in the corpus of public Moxfield and Archidekt decks parses to exactly the cards its site says the deck holds, so the parsers are checked against what the sites really write and not against fixtures we made up.
//!
//! The rules come from `tests/fixtures/corpus/README.md`: cards are compared as multisets of (section, name, set, collector number, count); `A / B` equals `A // B`; front names match the first face; `lossy` (Arena) means the text may only hold part of the deck; Moxfield flavor names are mapped to oracle names with the printing they share with the truth.
//! Beyond the comparison the file pins what the texts cannot tell: how commanders are told apart, that signature spells and stickers are not commanders, and what `promote_implied_commander` does with partner pairs.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod corpus;

use std::path::PathBuf;

use corpus::{Aliases, Deck, Marking, Site, check, observed, shape};
use mtg_import::decklist::{DeckFormat, Section, parse_as, promote_implied_commander};
use rstest::rstest;

/// Calls `visit(deck, export name, variant)` for every export whose commanders are marked `marking`.
fn for_exports_marked(marking: Marking, mut visit: impl FnMut(&Deck, &str)) {
    for deck in Deck::all() {
        for (name, variant) in &deck.truth.variants {
            if variant.commanders == marking {
                visit(&deck, name);
            }
        }
    }
}

fn commander_copies(deck: &Deck) -> u64 {
    deck.truth
        .boards
        .commanders
        .iter()
        .map(|r| u64::from(r.qty))
        .sum()
}

#[rstest]
fn every_export_of_a_deck_agrees_with_the_site(
    #[files("tests/fixtures/corpus/*/*/truth.json")] truth: PathBuf,
) {
    let deck = Deck::load(&truth);
    let aliases = Aliases::of(&deck);
    let failures: Vec<String> = deck
        .truth
        .variants
        .keys()
        .filter_map(|variant| check(&deck, variant, &aliases).err())
        .collect();
    assert!(failures.is_empty(), "\n{}", failures.join("\n"));
}

#[test]
fn the_table_covers_every_export_file_of_every_deck() {
    let decks = Deck::all();
    for site in [Site::Moxfield, Site::Archidekt] {
        let of_site = decks.iter().filter(|d| d.site == site).count();
        assert!(of_site >= 13, "{site}: only {of_site} decks were found");
    }
    for deck in &decks {
        let declared: Vec<&str> = deck.truth.variants.keys().map(String::as_str).collect();
        assert_eq!(declared, deck.export_files(), "{}", deck.label());
    }
}

/// A subset check passes for an empty parse, so a lossy export must also be read line for line.
#[test]
fn a_lossy_export_is_read_line_for_line() {
    let mut seen = 0;
    for deck in Deck::all() {
        for name in deck
            .truth
            .variants
            .iter()
            .filter(|(_, v)| v.lossy)
            .map(|(name, _)| name)
        {
            let text = deck.text(name);
            let parsed = parse_as(shape(deck.site, name).format, &text);
            let card_lines = text
                .lines()
                .filter(|l| l.starts_with(|c: char| c.is_ascii_digit()))
                .count();
            assert_eq!(parsed.entries.len(), card_lines, "{}/{name}", deck.label());
            assert!(card_lines > 0, "{}/{name}", deck.label());
            seen += 1;
        }
    }
    assert!(seen >= 20, "only {seen} lossy exports were found");
}

#[test]
fn windows_line_endings_change_nothing() {
    for deck in Deck::all() {
        for name in deck.truth.variants.keys() {
            let text = deck.text(name);
            let format = shape(deck.site, name).format;
            let crlf = text.replace('\n', "\r\n");
            assert_eq!(
                parse_as(format, &crlf),
                parse_as(format, &text),
                "{}/{name}",
                deck.label()
            );
        }
    }
}

/// Moxfield's `full` writes the commanders as the first lines, indistinguishable from the main deck.
#[test]
fn first_lines_commanders_stay_in_main_until_the_caller_promotes() {
    let mut seen = 0;
    for_exports_marked(Marking::FirstLines, |deck, name| {
        let boards = &deck.truth.boards;
        let parsed = parse_as(DeckFormat::Moxfield, &deck.text(name));
        let main_cards: u64 = [
            &boards.commanders,
            &boards.signature_spells,
            &boards.mainboard,
        ]
        .into_iter()
        .flatten()
        .map(|row| u64::from(row.qty))
        .sum();
        assert_eq!(
            parsed.card_count(Section::Commander),
            0,
            "{}/{name}",
            deck.label()
        );
        assert_eq!(
            parsed.card_count(Section::Main),
            main_cards,
            "{}/{name}",
            deck.label()
        );
        seen += 1;
    });
    assert!(seen >= 20, "only {seen} first-lines exports were found");
}

/// With several commanders the text cannot say how many lines they take; promoting moves the first line and leaves the rest to the deck builder.
#[test]
fn promoting_moves_a_single_first_line_so_partner_pairs_stay_a_ui_matter() {
    let mut partner_pairs = 0;
    for_exports_marked(Marking::FirstLines, |deck, name| {
        let parsed = parse_as(DeckFormat::Moxfield, &deck.text(name));
        let promoted = promote_implied_commander(DeckFormat::Moxfield, parsed.clone());
        let zone: Vec<_> = promoted
            .entries
            .iter()
            .filter(|e| e.section == Section::Commander)
            .collect();
        let label = format!("{}/{name}", deck.label());
        match parsed.entries.first() {
            Some(first) if first.count == 1 => {
                assert_eq!(zone.len(), 1, "{label}");
                assert_eq!(zone[0].name, first.name, "{label}");
                assert_eq!(promoted.entries[1..], parsed.entries[1..], "{label}");
            }
            _ => assert!(zone.is_empty(), "{label}"),
        }
        if deck.truth.boards.commanders.len() >= 2 {
            partner_pairs += 1;
            assert!(
                commander_copies(deck) > u64::try_from(zone.len()).unwrap(),
                "{label}: a pair must not be complete"
            );
        }
    });
    assert!(
        partner_pairs >= 8,
        "only {partner_pairs} partner-pair exports were found"
    );
}

/// MTGO text puts the commanders in a trailing block of their own, however many there are and whether or not a sideboard comes first.
#[test]
fn mtgo_text_marks_every_commander_by_its_trailing_block() {
    let mut seen = 0;
    for_exports_marked(Marking::LastBlock, |deck, name| {
        let parsed = parse_as(DeckFormat::MtgoText, &deck.text(name));
        let zone = parsed
            .entries
            .iter()
            .filter(|e| e.section == Section::Commander)
            .count();
        let label = format!("{}/{name}", deck.label());
        assert_eq!(zone, deck.truth.boards.commanders.len(), "{label}");
        assert_eq!(
            parsed.card_count(Section::Commander),
            commander_copies(deck),
            "{label}"
        );
        seen += 1;
    });
    assert!(seen >= 16, "only {seen} MTGO exports were found");
}

/// Oathbreaker signature spells are deck cards: they sit in Main in every export and are never promoted.
#[test]
fn signature_spells_stay_in_main() {
    let mut spells = 0;
    for deck in Deck::all() {
        let aliases = Aliases::of(&deck);
        for (name, variant) in &deck.truth.variants {
            let shape = shape(deck.site, name);
            let parsed = corpus::import(&deck, variant, shape, &deck.text(name));
            let cards = observed(&parsed, shape, &aliases);
            for row in &deck.truth.boards.signature_spells {
                let wanted = corpus::canonical(&row.name);
                let in_section = |section| {
                    cards
                        .iter()
                        .any(|(key, _)| key.section == section && key.name == wanted)
                };
                let label = format!("{}/{name}: {}", deck.label(), row.name);
                assert!(!in_section(Section::Commander), "{label}");
                assert!(variant.lossy || in_section(Section::Main), "{label}");
                spells += 1;
            }
        }
    }
    assert!(spells >= 4, "only {spells} signature spells were found");
}

/// Stickers are listed in the export but are not deck cards: they are reported, never added to a section.
#[test]
fn stickers_are_reported_by_name_and_count_and_kept_out_of_the_deck() {
    let mut with_stickers = 0;
    for deck in Deck::all() {
        let stickers: u64 = deck
            .truth
            .boards
            .stickers
            .iter()
            .map(|r| u64::from(r.qty))
            .sum();
        for name in deck.truth.variants.keys() {
            let text = deck.text(name);
            let parsed = parse_as(shape(deck.site, name).format, &text);
            let listed = text.lines().any(|l| l.trim() == "STICKERS:");
            let label = format!("{}/{name}", deck.label());
            assert_eq!(parsed.ignored.is_empty(), !listed, "{label}");
            if listed {
                assert_eq!(parsed.ignored.len(), 1, "{label}");
                assert_eq!(parsed.ignored[0].name, "STICKERS", "{label}");
                assert_eq!(parsed.ignored[0].count, stickers, "{label}");
                let sticker_names: Vec<&str> = deck
                    .truth
                    .boards
                    .stickers
                    .iter()
                    .map(|row| row.name.as_str())
                    .collect();
                assert!(
                    parsed
                        .entries
                        .iter()
                        .all(|e| !sticker_names.contains(&e.name.as_str())),
                    "{label}"
                );
                with_stickers += 1;
            }
        }
    }
    assert!(
        with_stickers >= 8,
        "only {with_stickers} exports with stickers were found"
    );
}

/// Flavor names are learned from the data, not listed: each is read from `full` at a printing the truth names differently.
#[test]
fn flavor_names_are_learned_from_the_data() {
    let mut learned = 0;
    for deck in Deck::all() {
        let aliases = Aliases::of(&deck);
        let text = deck.try_text("full").unwrap_or_default();
        for (flavor, oracle) in aliases.flavor_names() {
            assert!(text.contains(flavor.as_str()), "{}: {flavor}", deck.label());
            assert_ne!(flavor, oracle, "{}", deck.label());
            learned += 1;
        }
        if deck.try_text("full-noflavor").is_some() {
            assert!(
                !aliases.flavor_names().is_empty(),
                "{}: full-noflavor exists, so a flavor name must",
                deck.label()
            );
        }
    }
    assert!(learned >= 6, "only {learned} flavor names were learned");
}
