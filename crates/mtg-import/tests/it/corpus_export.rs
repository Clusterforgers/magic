//! Goal: every deck read from the corpus can be written out and read back unchanged, so the exporter copes with real names (accents, slashes, emoji) and collector numbers like `TMP-315` or `121★`, and drops the one thing it cannot hold, the sections that were never deck cards, on purpose.
//!
//! The plain-text exporter writes Arena-style headers and printings, no flags or categories; foil flags, categories and tags never reach a `ParsedDeck`, so only `ignored` is left out of the comparison. The Arena and MTGO writers are held to the same decks through the parsers of their own dialects.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::corpus;

use std::path::PathBuf;

use corpus::{Deck, import, shape};
use mtg_import::decklist::{
    DeckEntry, DeckFormat, ExportFormat, Section, detect, export, export_as, parse, parse_as,
};
use rstest::rstest;

#[rstest]
fn exporting_a_corpus_deck_and_parsing_it_back_changes_nothing(
    #[files("tests/fixtures/corpus/*/*/truth.json")] truth: PathBuf,
) {
    let deck = Deck::load(&truth);
    for (name, variant) in &deck.truth.variants {
        let imported = import(&deck, variant, shape(deck.site, name), &deck.text(name));
        let label = format!("{}/{name}", deck.label());
        let text = export(&imported).unwrap_or_else(|e| panic!("{label}: {e}"));
        let again = parse(&text);
        assert_eq!(again.entries, imported.entries, "{label}");
        assert!(again.errors.is_empty(), "{label}: {:?}", again.errors);
        assert!(again.ignored.is_empty(), "{label}");
        assert!(
            matches!(detect(&text), DeckFormat::PlainText | DeckFormat::Arena),
            "{label}: exported text must not look like a richer format"
        );
        assert_eq!(
            export(&again).unwrap(),
            text,
            "{label}: export must be stable"
        );
    }
}

/// The entries in the order a writer lists the sections.
fn by_section(entries: &[DeckEntry], order: &[Section]) -> Vec<DeckEntry> {
    order
        .iter()
        .flat_map(|section| entries.iter().filter(move |e| e.section == *section))
        .cloned()
        .collect()
}

#[rstest]
fn the_arena_and_mtgo_writers_read_back_through_their_own_parsers(
    #[files("tests/fixtures/corpus/*/*/truth.json")] truth: PathBuf,
) {
    let deck = Deck::load(&truth);
    for (name, variant) in &deck.truth.variants {
        let imported = import(&deck, variant, shape(deck.site, name), &deck.text(name));
        let label = format!("{}/{name}", deck.label());

        let arena =
            export_as(ExportFormat::Arena, &imported).unwrap_or_else(|e| panic!("{label}: {e}"));
        let again = parse_as(DeckFormat::Arena, &arena);
        assert!(again.errors.is_empty(), "{label}: {:?}", again.errors);
        let order = [
            Section::Commander,
            Section::Companion,
            Section::Main,
            Section::Sideboard,
        ];
        assert_eq!(
            again.entries,
            by_section(&imported.entries, &order),
            "{label}"
        );
        if imported
            .entries
            .iter()
            .any(|e| e.section != Section::Sideboard)
        {
            assert_eq!(detect(&arena), DeckFormat::Arena, "{label}");
        }

        // MTGO text has no printings and no companion zone, and marks its commanders by position:
        // a list that has commanders and nothing else cannot be written.
        match export_as(ExportFormat::Mtgo, &imported) {
            Ok(text) => {
                let again = parse_as(DeckFormat::MtgoText, &text);
                assert!(again.errors.is_empty(), "{label}: {:?}", again.errors);
                let kept = by_section(
                    &imported.entries,
                    &[
                        Section::Main,
                        Section::Sideboard,
                        Section::Companion,
                        Section::Commander,
                    ],
                );
                assert_eq!(
                    again
                        .entries
                        .iter()
                        .map(|e| (e.count, e.name.as_str()))
                        .collect::<Vec<_>>()
                        .len(),
                    kept.len(),
                    "{label}"
                );
                let total = |entries: &[DeckEntry]| entries.iter().map(|e| e.count).sum::<u32>();
                assert_eq!(total(&again.entries), total(&kept), "{label}");
            }
            Err(error) => {
                let only_commanders = imported
                    .entries
                    .iter()
                    .all(|e| e.section == Section::Commander);
                assert!(only_commanders, "{label}: {error}");
            }
        }
    }
}
