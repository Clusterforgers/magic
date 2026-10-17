//! Goal: every deck read from the corpus can be written out and read back unchanged, so the exporter copes with real names (accents, slashes, emoji) and collector numbers like `TMP-315` or `121★`, and drops the one thing it cannot hold, the sections that were never deck cards, on purpose.
//!
//! The plain-text exporter is the only one there is (Arena-style headers, printings, no flags or categories); foil flags, categories and tags never reach a `ParsedDeck`, so only `ignored` is left out of the comparison.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod corpus;

use std::path::PathBuf;

use corpus::{Deck, import, shape};
use mtg_import::decklist::{DeckFormat, detect, export, parse};
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
