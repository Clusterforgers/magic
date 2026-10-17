//! Goal: every export in the corpus of public Moxfield and Archidekt decks parses to exactly the cards its site says the deck holds, so the parsers are checked against what the sites really write and not against fixtures we made up.
//!
//! The rules come from `tests/fixtures/corpus/README.md`: cards are compared as multisets of (section, name, set, collector number, count); `A / B` equals `A // B`; front names match the first face; `lossy` (Arena) means the text may only hold part of the deck; Moxfield flavor names are mapped to oracle names with the printing they share with the truth.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod corpus;

use std::path::PathBuf;

use corpus::{Aliases, Deck, Site, check};
use rstest::rstest;

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
