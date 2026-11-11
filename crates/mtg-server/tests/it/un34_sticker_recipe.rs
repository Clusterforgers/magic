//! Goal: a recipe names a seat's sticker sheets and rebuilds the setup with them at two to four seats, mixed with seats that have none; the sheets are part of the seat's fingerprint, so a changed sheet fails the rebuild, and a recipe stored without sheets reads and hashes as it did.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeMap;
use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Sticker;
use mtg_server::{Catalog, DeckFingerprint, DeckRef, GameRecipe, RecipeError};
use mtg_testkit::cards::{free_creature, un34_sheet};
use rstest::rstest;

const SHEETS: [&str; 2] = ["sheet-a", "sheet-b"];

fn sheet(name: &str, words: &str) -> Arc<CardDefinition> {
    un34_sheet(
        name,
        vec![Sticker::Name {
            words: words.to_owned(),
            tickets: 1,
        }],
    )
}

fn cards() -> BTreeMap<String, Arc<CardDefinition>> {
    let mut cards: BTreeMap<String, Arc<CardDefinition>> = (0..20)
        .map(|n| {
            (
                format!("bear-{n}"),
                free_creature(&format!("Bear {n}"), 2, 2),
            )
        })
        .collect();
    cards.insert(SHEETS[0].to_owned(), sheet("Sheet A", "Wicked"));
    cards.insert(SHEETS[1].to_owned(), sheet("Sheet B", "Fluffy"));
    cards
}

fn catalog_of(cards: &BTreeMap<String, Arc<CardDefinition>>) -> Catalog {
    Catalog::new(
        cards
            .iter()
            .map(|(id, card)| (id.clone(), Arc::clone(card))),
    )
}

fn oracle(stickers: Vec<String>) -> DeckRef {
    DeckRef::Oracle {
        cards: (0..20).map(|n| format!("bear-{n}")).collect(),
        commanders: Vec::new(),
        companion: None,
        printings: Vec::new(),
        by_hand: Vec::new(),
        attractions: Vec::new(),
        stickers,
    }
}

/// The even seats bring both sheets, the others none.
fn recipe(seats: usize, catalog: &Catalog) -> GameRecipe {
    let decks = (0..seats).map(|seat| {
        oracle(if seat % 2 == 0 {
            SHEETS.iter().map(|id| (*id).to_owned()).collect()
        } else {
            Vec::new()
        })
    });
    GameRecipe::normal(7, decks)
        .with_fingerprints_from(catalog)
        .unwrap()
}

#[rstest]
fn a_recipe_rebuilds_the_setup_with_the_sticker_sheets(#[values(2, 3, 4)] seats: usize) {
    let cards = cards();
    let catalog = catalog_of(&cards);
    let recipe = recipe(seats, &catalog);
    let back: GameRecipe = serde_json::from_str(&serde_json::to_string(&recipe).unwrap()).unwrap();
    assert_eq!(back, recipe);
    let setup = back.build_with(&catalog).unwrap();
    for (seat, player) in setup.players.iter().enumerate() {
        let names: Vec<&str> = player
            .stickers
            .iter()
            .map(|sheet| sheet.name.as_str())
            .collect();
        let expected: &[&str] = if seat % 2 == 0 {
            &["Sheet A", "Sheet B"]
        } else {
            &[]
        };
        assert_eq!(names, expected, "seat {seat}");
        assert_eq!(
            player.library.len(),
            20,
            "the sheets are no cards of the deck"
        );
    }
}

#[test]
fn a_seat_without_sheets_hashes_as_before_and_a_changed_sheet_fails() {
    let mut cards = cards();
    let catalog = catalog_of(&cards);
    let without = GameRecipe::normal(7, [oracle(Vec::new())])
        .with_fingerprints_from(&catalog)
        .unwrap();
    let library: Vec<_> = (0..20)
        .map(|n| Arc::clone(&cards[&format!("bear-{n}")]))
        .collect();
    assert_eq!(
        without.fingerprints[0],
        DeckFingerprint::of(&library).unwrap()
    );
    let with = recipe(2, &catalog);
    assert_ne!(with.fingerprints[0], with.fingerprints[1]);
    cards.insert(SHEETS[1].to_owned(), sheet("Sheet B", "Grumpy"));
    let changed = catalog_of(&cards);
    assert!(matches!(
        with.build_with(&changed),
        Err(RecipeError::DeckChanged { seat: 0, .. })
    ));
}
