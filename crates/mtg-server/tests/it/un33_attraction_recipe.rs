//! Goal: a recipe names a seat's Attraction deck with the lights of each card and rebuilds the setup with it at two to four seats, mixed with seats that have none; the deck and its lights are part of the seat's fingerprint, so changed lights, a dropped card or a changed card fail the rebuild with the seat named, and a recipe stored without an Attraction deck reads and hashes as it did.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeMap;
use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_server::{AttractionRef, Catalog, DeckFingerprint, DeckRef, GameRecipe, RecipeError};
use mtg_testkit::cards::{free_creature, un33_attraction};
use rstest::rstest;
use serde_json::json;

const RIDES: [&str; 3] = ["ride-a", "ride-b", "ride-c"];

fn cards() -> BTreeMap<String, Arc<CardDefinition>> {
    let mut cards: BTreeMap<String, Arc<CardDefinition>> = (0..20)
        .map(|n| (format!("bear-{n}"), free_creature(&format!("Bear {n}"), 2, 2)))
        .collect();
    for (at, id) in RIDES.iter().enumerate() {
        cards.insert((*id).to_owned(), un33_attraction(&format!("Ride {at}")));
    }
    cards
}

fn catalog_of(cards: &BTreeMap<String, Arc<CardDefinition>>) -> Catalog {
    Catalog::new(cards.iter().map(|(id, card)| (id.clone(), Arc::clone(card))))
}

fn library() -> Vec<String> {
    (0..20).map(|n| format!("bear-{n}")).collect()
}

fn rides() -> Vec<AttractionRef> {
    RIDES
        .iter()
        .zip([vec![2, 6], vec![3, 6], vec![4, 5, 6]])
        .map(|(id, lights)| AttractionRef {
            oracle_id: (*id).to_owned(),
            lights,
        })
        .collect()
}

fn oracle(attractions: Vec<AttractionRef>) -> DeckRef {
    DeckRef::Oracle {
        cards: library(),
        commanders: Vec::new(),
        companion: None,
        printings: Vec::new(),
        by_hand: Vec::new(),
        attractions,
    }
}

/// Seats 0 and 2 bring the Attraction deck, the others none.
fn recipe(seats: usize, catalog: &Catalog) -> GameRecipe {
    let decks = (0..seats).map(|seat| oracle(if seat % 2 == 0 { rides() } else { Vec::new() }));
    GameRecipe::normal(7, decks)
        .with_fingerprints_from(catalog)
        .unwrap()
}

#[rstest]
fn a_recipe_rebuilds_the_setup_with_the_attraction_decks(#[values(2, 3, 4)] seats: usize) {
    let cards = cards();
    let catalog = catalog_of(&cards);
    let recipe = recipe(seats, &catalog);
    let back: GameRecipe = serde_json::from_str(&serde_json::to_string(&recipe).unwrap()).unwrap();
    assert_eq!(back, recipe);
    let setup = back.build_with(&catalog).unwrap();
    for (seat, player) in setup.players.iter().enumerate() {
        let deck: Vec<(String, Vec<u8>)> = player
            .attractions
            .iter()
            .map(|card| (card.card.name.clone(), card.lights.numbers()))
            .collect();
        let expected = if seat % 2 == 0 {
            vec![
                ("Ride 0".to_owned(), vec![2, 6]),
                ("Ride 1".to_owned(), vec![3, 6]),
                ("Ride 2".to_owned(), vec![4, 5, 6]),
            ]
        } else {
            Vec::new()
        };
        assert_eq!(deck, expected, "seat {seat}");
        assert_eq!(player.library.len(), 20, "the Attractions are no cards of the deck");
    }
}

#[test]
fn the_stored_shape_names_the_deck_and_a_seat_without_one_keeps_the_shape_it_had() {
    let cards = cards();
    let catalog = catalog_of(&cards);
    let stored = serde_json::to_value(recipe(2, &catalog)).unwrap();
    assert_eq!(
        stored["decks"][0]["attractions"][0],
        json!({"oracle_id": "ride-a", "lights": [2, 6]})
    );
    assert!(stored["decks"][1].get("attractions").is_none());
    let without = GameRecipe::normal(7, [oracle(Vec::new())])
        .with_fingerprints_from(&catalog)
        .unwrap();
    let library: Vec<_> = library().iter().map(|id| Arc::clone(&cards[id])).collect();
    assert_eq!(without.fingerprints[0], DeckFingerprint::of(&library).unwrap());
}

type Edit = fn(&mut GameRecipe, &mut BTreeMap<String, Arc<CardDefinition>>);

fn relit(recipe: &mut GameRecipe, _: &mut BTreeMap<String, Arc<CardDefinition>>) {
    if let DeckRef::Oracle { attractions, .. } = &mut recipe.decks[0] {
        attractions[0].lights = vec![2, 5];
    }
}

fn dropped(recipe: &mut GameRecipe, _: &mut BTreeMap<String, Arc<CardDefinition>>) {
    if let DeckRef::Oracle { attractions, .. } = &mut recipe.decks[0] {
        attractions.pop();
    }
}

fn changed_card(_: &mut GameRecipe, cards: &mut BTreeMap<String, Arc<CardDefinition>>) {
    cards.insert("ride-b".to_owned(), un33_attraction("Another Ride"));
}

fn added(recipe: &mut GameRecipe, _: &mut BTreeMap<String, Arc<CardDefinition>>) {
    if let DeckRef::Oracle { attractions, .. } = &mut recipe.decks[1] {
        *attractions = rides();
    }
}

#[rstest]
#[case::other_lights(relit as Edit, 0)]
#[case::a_card_is_gone(dropped as Edit, 0)]
#[case::the_card_was_edited(changed_card as Edit, 0)]
#[case::a_seat_gained_one(added as Edit, 1)]
fn a_changed_attraction_deck_fails_the_rebuild_with_the_seat_named(
    #[case] edit: Edit,
    #[case] seat: usize,
    #[values(2, 3, 4)] seats: usize,
) {
    let mut cards = cards();
    let mut recipe = recipe(seats, &catalog_of(&cards));
    edit(&mut recipe, &mut cards);
    match recipe.build_with(&catalog_of(&cards)).unwrap_err() {
        RecipeError::DeckChanged { seat: failed, .. } => assert_eq!(failed, seat),
        other => panic!("{other}"),
    }
}
