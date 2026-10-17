//! Goal: the parsers and resolver cope with the real Scryfall and MTGJSON files, not only hand-written fixtures.
//!
//! Ignored by default (needs downloads). Set `MTG_REAL_DATA_DIR` to a directory holding
//! `oracle-cards-*.jsonl.gz`, `rulings-*.jsonl.gz`, `DeckList.json` and one or more MTGJSON deck files named
//! `*_<SET>.json`, then run `cargo test -p mtg-import --test real_data -- --ignored`.

use std::fs::{self, File};
use std::io::BufReader;
use std::path::PathBuf;

use flate2::read::GzDecoder;
use mtg_import::mtgjson::{parse_deck, parse_deck_list};
use mtg_import::resolve::{CardIndex, resolve_deck};
use mtg_import::scryfall::{CardRecord, cards_jsonl, rulings_jsonl};

fn data_dir() -> PathBuf {
    PathBuf::from(std::env::var("MTG_REAL_DATA_DIR").expect("set MTG_REAL_DATA_DIR"))
}

fn files_matching(prefix: &str, suffix: &str) -> Vec<PathBuf> {
    let mut found: Vec<PathBuf> = fs::read_dir(data_dir())
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| {
            let name = path.file_name().unwrap().to_string_lossy();
            name.starts_with(prefix) && name.ends_with(suffix)
        })
        .collect();
    found.sort();
    found
}

fn load_cards() -> Vec<CardRecord> {
    let path = files_matching("oracle-cards-", ".jsonl.gz")
        .pop()
        .expect("oracle cards file");
    let reader = GzDecoder::new(BufReader::new(File::open(path).unwrap()));
    cards_jsonl(reader)
        .map(|card| card.unwrap_or_else(|e| panic!("real card failed to parse: {e}")))
        .collect()
}

#[test]
#[ignore = "needs MTG_REAL_DATA_DIR"]
fn every_real_deck_card_parses_with_usable_data() {
    let records = load_cards();
    assert!(records.len() > 25_000, "only {} cards", records.len());
    let problems: Vec<String> = records
        .iter()
        .filter(|card| card.layout.is_deck_card())
        .flat_map(|card| {
            let missing_images = (0..card.faces().count())
                .filter(|&i| {
                    card.face_images(i)
                        .and_then(|images| images.normal.as_ref())
                        .is_none()
                })
                .map(|i| format!("{}: no image for face {i}", card.name));
            let missing_type = card
                .front
                .type_line
                .is_empty()
                .then(|| format!("{}: no type line", card.name));
            missing_images.chain(missing_type).collect::<Vec<_>>()
        })
        .collect();
    assert!(
        problems.is_empty(),
        "{} problems, first: {:?}",
        problems.len(),
        &problems[..problems.len().min(5)]
    );
}

#[test]
#[ignore = "needs MTG_REAL_DATA_DIR"]
fn every_real_ruling_parses() {
    let path = files_matching("rulings-", ".jsonl.gz")
        .pop()
        .expect("rulings file");
    let reader = GzDecoder::new(BufReader::new(File::open(path).unwrap()));
    let count = rulings_jsonl(reader)
        .map(|ruling| ruling.unwrap_or_else(|e| panic!("real ruling failed to parse: {e}")))
        .count();
    assert!(count > 40_000, "only {count} rulings");
}

#[test]
#[ignore = "needs MTG_REAL_DATA_DIR"]
fn real_deck_index_lists_commander_precons() {
    let reader = File::open(data_dir().join("DeckList.json")).unwrap();
    let decks = parse_deck_list(BufReader::new(reader)).unwrap();
    assert!(decks.len() > 1_000);
    assert!(decks.iter().filter(|d| d.is_commander_precon()).count() > 50);
}

#[test]
#[ignore = "needs MTG_REAL_DATA_DIR"]
fn real_precon_decks_resolve_completely_against_real_cards() {
    let index = CardIndex::from_records(&load_cards());
    let decks = files_matching("", ".json")
        .into_iter()
        .filter(|path| path.file_name().unwrap().to_string_lossy().contains('_'))
        .collect::<Vec<_>>();
    assert!(!decks.is_empty(), "no MTGJSON deck files in the data dir");
    for path in decks {
        let deck = parse_deck(BufReader::new(File::open(&path).unwrap())).unwrap();
        let parsed = deck.to_parsed_deck();
        let resolution = resolve_deck(&index, &parsed);
        assert!(
            resolution.unresolved.is_empty(),
            "{}: unresolved {:?}",
            deck.name,
            resolution
                .unresolved
                .iter()
                .map(|u| (&u.entry.name, &u.error))
                .collect::<Vec<_>>()
        );
        let resolved: u32 = resolution.resolved.iter().map(|r| r.count).sum();
        let listed: u32 = parsed.entries.iter().map(|e| e.count).sum();
        assert!(
            listed > 0 && resolved == listed,
            "{}: resolved {resolved} of {listed} cards",
            deck.name
        );
    }
}
