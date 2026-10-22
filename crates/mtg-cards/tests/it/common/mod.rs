//! Goal: shared helpers for the integration tests: the corpus directory, the directory of imported Scryfall fixtures and the checks of the corpus that derive every count from them (this is not a test module itself).
#![allow(dead_code, clippy::expect_used, clippy::unwrap_used)]

use std::path::{Path, PathBuf};

use mtg_cards::load::Source;
use mtg_cards::support::{PrintedCard, PrintedFace};
use mtg_core::mana::ManaType;
use mtg_import::scryfall::CardRecord;

pub mod checks;

pub use checks::{ADDING, Corpus, UPDATE, update_repo_once};

/// The directory of Scryfall objects (Oracle Cards records), one `<slug>.json` file per card of the corpus.
pub fn fixture_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/corpus")
}

/// The directory of rules files.
pub fn cards_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("cards")
}

/// The predefined tokens (`cards/tokens/`, whichever files are there) that a compiled card may name.
pub fn token_sources() -> Vec<Source> {
    Corpus::repo().token_sources()
}

/// An imported record as the strings validation compares with.
pub fn printed_of(record: &CardRecord) -> PrintedCard {
    PrintedCard {
        layout: record.layout.keyword().to_owned(),
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
                colors: face
                    .colors
                    .iter()
                    .map(|color| ManaType::from(color).symbol().to_string())
                    .collect(),
            })
            .collect(),
    }
}

/// The imported records of the corpus, as the importer reads them.
pub fn records() -> Vec<CardRecord> {
    Corpus::repo().records()
}

/// The imported cards of the corpus.
pub fn printed() -> Vec<PrintedCard> {
    Corpus::repo().printed()
}
