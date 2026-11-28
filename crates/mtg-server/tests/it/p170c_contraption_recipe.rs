//! Goal: a recipe names a seat's Contraption deck and rebuilds the setup with it at two to four seats, mixed with seats that have none; the deck is part of the seat's fingerprint, so a changed Contraption fails the rebuild, and a recipe stored without one reads and hashes as it did.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeMap;
use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_server::{Catalog, DeckFingerprint, DeckRef, GameRecipe, RecipeError};
use mtg_testkit::cards::{free_creature, p170b_contraption};
use rstest::rstest;

const GIZMOS: [&str; 3] = ["gizmo-a", "gizmo-b", "gizmo-c"];

fn cards() -> BTreeMap<String, Arc<CardDefinition>> {
    let mut cards: BTreeMap<String, Arc<CardDefinition>> = (0..20)
        .map(|n| {
            (
                format!("bear-{n}"),
                free_creature(&format!("Bear {n}"), 2, 2),
            )
        })
        .collect();
    for (id, name) in GIZMOS.iter().zip(["Gizmo A", "Gizmo B", "Gizmo C"]) {
        cards.insert((*id).to_owned(), p170b_contraption(name));
    }
    cards
}

fn catalog_of(cards: &BTreeMap<String, Arc<CardDefinition>>) -> Catalog {
    Catalog::new(
        cards
            .iter()
            .map(|(id, card)| (id.clone(), Arc::clone(card))),
    )
}

fn oracle(contraptions: Vec<String>) -> DeckRef {
    DeckRef::Oracle {
        cards: (0..20).map(|n| format!("bear-{n}")).collect(),
        commanders: Vec::new(),
        companion: None,
        printings: Vec::new(),
        by_hand: Vec::new(),
        attractions: Vec::new(),
        contraptions,
        stickers: Vec::new(),
        satchel: Vec::new(),
        note: None,
    }
}

/// The even seats bring the three Contraptions, the others none.
fn recipe(seats: usize, catalog: &Catalog) -> GameRecipe {
    let decks = (0..seats).map(|seat| {
        oracle(if seat % 2 == 0 {
            GIZMOS.iter().map(|id| (*id).to_owned()).collect()
        } else {
            Vec::new()
        })
    });
    GameRecipe::normal(7, decks)
        .with_fingerprints_from(catalog)
        .unwrap()
}

#[rstest]
fn a_recipe_rebuilds_the_setup_with_the_contraption_deck(#[values(2, 3, 4)] seats: usize) {
    let cards = cards();
    let catalog = catalog_of(&cards);
    let recipe = recipe(seats, &catalog);
    let back: GameRecipe = serde_json::from_str(&serde_json::to_string(&recipe).unwrap()).unwrap();
    assert_eq!(back, recipe);
    assert!(back.oracle_ids().iter().any(|id| id == GIZMOS[0]));
    let setup = back.build_with(&catalog).unwrap();
    for (seat, player) in setup.players.iter().enumerate() {
        let names: Vec<&str> = player
            .contraptions
            .iter()
            .map(|card| card.name.as_str())
            .collect();
        let expected: &[&str] = if seat % 2 == 0 {
            &["Gizmo A", "Gizmo B", "Gizmo C"]
        } else {
            &[]
        };
        assert_eq!(names, expected, "seat {seat}");
        assert_eq!(player.library.len(), 20, "the Contraptions are no cards of the deck");
    }
}

#[test]
fn a_seat_without_contraptions_hashes_as_before_and_a_changed_contraption_fails() {
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
    let stored = serde_json::to_string(&oracle(Vec::new())).unwrap();
    assert!(!stored.contains("contraptions"), "{stored}");
    let with = recipe(2, &catalog);
    assert_ne!(with.fingerprints[0], with.fingerprints[1]);
    cards.insert(GIZMOS[1].to_owned(), p170b_contraption("Gizmo Z"));
    let changed = catalog_of(&cards);
    assert!(matches!(
        with.build_with(&changed),
        Err(RecipeError::DeckChanged { seat: 0, .. })
    ));
}
