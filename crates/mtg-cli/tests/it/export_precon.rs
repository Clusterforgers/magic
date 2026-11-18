//! Goal: `export-precon` writes a stored precon as a content deck file: the commander shuffled in as an ordinary card,
//! the sideboard left out, each swap moving every copy onto its basic land, and a swap of a card the deck lacks refused.

use crate::common;

use mtg_cli::args::ExportPreconArgs;
use mtg_cli::export_precon::{Swap, deck_file, export_precon, file_name};
use mtg_cli::ingest::{IngestOptions, ingest};
use mtg_db::{Database, DeckBoard, PreconDeckDetail, PreconDeckId, PreconDeckSummary, PreconEntry};
use rstest::rstest;
use serde_json::{Value, json};
use sqlx::PgPool;

/// A precon of these `(board, count, name)` lines.
fn precon(lines: &[(DeckBoard, u32, &str)]) -> PreconDeckDetail {
    PreconDeckDetail {
        summary: PreconDeckSummary {
            id: PreconDeckId(1),
            code: "BLC".to_owned(),
            name: "Squirreled Away".to_owned(),
            deck_type: Some("Commander Deck".to_owned()),
            release_date: None,
            card_count: lines.iter().map(|line| u64::from(line.1)).sum(),
            unresolved_count: 0,
        },
        entries: lines
            .iter()
            .zip(0..)
            .map(|(&(board, count, name), position)| PreconEntry {
                position,
                board,
                count,
                card_name: name.to_owned(),
                is_foil: false,
                printing_id: None,
                set_code: None,
                collector_number: None,
                card: None,
            })
            .collect(),
    }
}

fn swaps(pairs: &[(&str, &str)]) -> Vec<Swap> {
    pairs
        .iter()
        .map(|(out, into)| Swap::parse(&format!("{out}={into}")).unwrap())
        .collect()
}

fn named(cards: &[(&str, u32)]) -> Vec<(String, u32)> {
    cards.iter().map(|(name, count)| ((*name).to_owned(), *count)).collect()
}

const DECK: &[(DeckBoard, u32, &str)] = &[
    (DeckBoard::Commander, 1, "Hazel of the Rootbloom"),
    (DeckBoard::Main, 1, "Illusionist's Gambit"),
    (DeckBoard::Main, 2, "Stalking Leonin"),
    (DeckBoard::Main, 10, "Forest"),
    (DeckBoard::Main, 9, "Island"),
    (DeckBoard::Sideboard, 1, "Sol Ring"),
];

#[rstest]
#[case::no_swap(&[], &[("Hazel of the Rootbloom", 1), ("Illusionist's Gambit", 1), ("Stalking Leonin", 2), ("Forest", 10), ("Island", 9)])]
#[case::onto_a_basic_the_deck_has(&[("Illusionist's Gambit", "Island")], &[("Hazel of the Rootbloom", 1), ("Stalking Leonin", 2), ("Forest", 10), ("Island", 10)])]
#[case::every_copy_onto_a_new_basic(&[("stalking leonin", "Plains")], &[("Hazel of the Rootbloom", 1), ("Illusionist's Gambit", 1), ("Forest", 10), ("Island", 9), ("Plains", 2)])]
#[case::two_swaps(&[("Illusionist's Gambit", "Island"), ("Stalking Leonin", "Forest")], &[("Hazel of the Rootbloom", 1), ("Forest", 12), ("Island", 10)])]
fn the_deck_file_holds_commander_and_main_with_the_swaps(
    #[case] given: &[(&str, &str)],
    #[case] expected: &[(&str, u32)],
) {
    let deck = deck_file(&precon(DECK), &swaps(given)).unwrap();
    assert_eq!(deck.cards, named(expected));
    assert_eq!(deck.commander, vec!["Hazel of the Rootbloom".to_owned()]);
    assert_eq!(deck.size(), 22, "the sideboard is left out, a swap keeps the count");
    assert_eq!(deck.swaps, swaps(given));
}

#[rstest]
#[case::a_card_the_deck_lacks("Sol Ring=Island")]
#[case::a_card_already_swapped("Illusionist's Gambit=Island")]
fn a_swap_of_a_missing_card_is_refused(#[case] second: &str) {
    let given = vec![
        Swap::parse("Illusionist's Gambit=Island").unwrap(),
        Swap::parse(second).unwrap(),
    ];
    let error = deck_file(&precon(DECK), &given).unwrap_err().to_string();
    assert!(error.contains("to swap"), "{error}");
}

#[rstest]
#[case::no_sign("Illusionist's Gambit")]
#[case::no_new_card("Illusionist's Gambit=")]
#[case::no_old_card("=Island")]
fn a_swap_reads_old_equals_new(#[case] text: &str) {
    assert!(Swap::parse(text).is_err());
}

#[rstest]
#[case::plain("BLC", "Squirreled Away", "blc-squirreled-away.json")]
#[case::punctuation("LCC", "Veloci-Ramp-Tor", "lcc-veloci-ramp-tor.json")]
#[case::apostrophe("C17", "Feline Ferocity", "c17-feline-ferocity.json")]
fn the_file_is_named_by_code_and_slug(#[case] code: &str, #[case] name: &str, #[case] file: &str) {
    assert_eq!(file_name(code, name), file);
}

#[sqlx::test]
async fn a_stored_precon_is_written_with_its_swaps(pool: PgPool) {
    let dir = tempfile::tempdir().unwrap();
    let cache = common::fixture_cache(dir.path());
    let db = Database::from_pool(pool);
    let (progress, _) = common::progress();
    let options = IngestOptions {
        force: false,
        batch_size: 500,
    };
    ingest(&cache, &db, &options, &progress).await.unwrap();
    let out = dir.path().join("decks");
    let args = ExportPreconArgs {
        code: "c16".to_owned(),
        name: "Breed Lethality".to_owned(),
        swap: vec!["Sol Ring=Forest".to_owned()],
        out: out.clone(),
    };
    let said = export_precon(&db, &args).await.unwrap();
    assert!(said.ends_with(": 40 cards"), "{said}");
    let text = std::fs::read_to_string(out.join("c16-breed-lethality.json")).unwrap();
    assert!(text.ends_with("}\n"));
    let file: Value = serde_json::from_str(&text).unwrap();
    assert_eq!(
        file,
        json!({
            "code": "C16",
            "name": "Breed Lethality",
            "commander": ["Atraxa, Praetors' Voice"],
            "swaps": [{"out": "Sol Ring", "into": "Forest"}],
            "cards": [["Atraxa, Praetors' Voice", 1], ["Arcane Signet", 1], ["Forest", 37], ["Fire // Ice", 1]],
        })
    );
    let missing = ExportPreconArgs {
        name: "No Such Deck".to_owned(),
        ..args
    };
    assert!(export_precon(&db, &missing).await.is_err());
}
