//! Goal: builders for small precon decks and a one-call ingest, shared by the precon tests.

use mtg_import::mtgjson::{PreconCard, PreconDeck};

/// A deck entry with no printing or ids, just a name and a count.
pub fn entry(name: &str, count: u32) -> PreconCard {
    PreconCard {
        count,
        uuid: format!("uuid-{name}"),
        name: name.to_owned(),
        set_code: None,
        number: None,
        is_foil: false,
        scryfall_id: None,
        oracle_id: None,
    }
}

pub fn deck(name: &str, code: &str, release: Option<&str>, main: Vec<PreconCard>) -> PreconDeck {
    PreconDeck {
        name: name.to_owned(),
        code: code.to_owned(),
        deck_type: Some("Test Deck".to_owned()),
        release_date: release.map(str::to_owned),
        main_board: main,
        side_board: Vec::new(),
        commander: Vec::new(),
        display_commander: Vec::new(),
    }
}

pub async fn ingest(
    database: &mtg_db::Database,
    decks: Vec<PreconDeck>,
) -> mtg_db::import::PreconImportReport {
    database
        .importer()
        .import_precons(decks.into_iter().map(Ok))
        .await
        .unwrap_or_else(|e| panic!("precon import failed: {e}"))
}
