//! Helpers shared by the integration tests: the corpus directory and the imported Scryfall fixture.
#![allow(dead_code, clippy::expect_used)]

use std::path::{Path, PathBuf};

use mtg_cards::support::{PrintedCard, PrintedFace};
use mtg_import::scryfall::{CardRecord, cards};

/// Scryfall objects (trimmed) of every card in the corpus, from the 2026-10-18 Oracle Cards bulk file.
const FIXTURE: &str = include_str!("../fixtures/corpus_scryfall.json");

/// The directory of rules files.
pub fn cards_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("cards")
}

/// An imported record as the strings validation compares with.
pub fn printed_of(record: &CardRecord) -> PrintedCard {
    PrintedCard {
        oracle_id: record.oracle_id.to_string(),
        name: record.name.clone(),
        keywords: record.keywords.clone(),
        faces: record
            .faces()
            .map(|face| PrintedFace {
                name: face.name.clone(),
                mana_cost: face.mana_cost.clone(),
                type_line: face.type_line.clone(),
                oracle_text: face.oracle_text.clone(),
                power: face.power.clone(),
                toughness: face.toughness.clone(),
                loyalty: face.loyalty.clone(),
                defense: face.defense.clone(),
            })
            .collect(),
    }
}

/// The imported cards of the corpus.
pub fn printed() -> Vec<PrintedCard> {
    cards(FIXTURE.as_bytes())
        .map(|record| printed_of(&record.expect("the fixture holds only valid records")))
        .collect()
}
