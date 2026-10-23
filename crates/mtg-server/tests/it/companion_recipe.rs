//! Goal: a recipe names a seat's companion beside its library and commanders and rebuilds the setup with it at two to four seats, mixed with seats that have none; the companion is part of the deck's fingerprint, so a changed, lost, swapped or dropped companion fails the rebuild with the seat named, a deck that no longer meets its condition is refused by the engine's setup check, and a recipe stored without a companion reads and hashes as it did.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeMap;
use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_engine::error::SetupError;
use mtg_engine::rng::GameRng;
use mtg_server::{Catalog, DeckFingerprint, DeckRef, GameRecipe, RecipeError};
use mtg_testkit::cards::{creature, land};
use mtg_testkit::companion_cards::{companion_with, small_companion, small_deck, small_permanents};
use rstest::rstest;
use serde_json::json;

const COMPANION: &str = "companion-small";

/// The 60 cards of a deck of small permanents, named by their card names, and the catalog that has them
/// and the companion.
fn deck_and_catalog() -> (Vec<String>, BTreeMap<String, Arc<CardDefinition>>) {
    let cards = small_deck(&mut GameRng::substream(9, 3));
    let mut catalog = BTreeMap::new();
    let ids = cards
        .iter()
        .map(|card| {
            catalog
                .entry(card.name.clone())
                .or_insert_with(|| Arc::clone(card));
            card.name.clone()
        })
        .collect();
    catalog.insert(COMPANION.to_owned(), small_companion());
    (ids, catalog)
}

fn catalog_of(cards: &BTreeMap<String, Arc<CardDefinition>>) -> Catalog {
    Catalog::new(
        cards
            .iter()
            .map(|(id, card)| (id.clone(), Arc::clone(card))),
    )
}

fn oracle(ids: &[String], companion: Option<&str>) -> DeckRef {
    DeckRef::Oracle {
        cards: ids.to_vec(),
        commanders: Vec::new(),
        companion: companion.map(str::to_owned),
    }
}

/// Seats 0, 2 and 4 name the companion, the others do not.
fn recipe(seats: usize, ids: &[String], catalog: &Catalog) -> GameRecipe {
    let decks = (0..seats).map(|seat| oracle(ids, (seat % 2 == 0).then_some(COMPANION)));
    GameRecipe::normal(7, decks)
        .with_fingerprints_from(catalog)
        .unwrap()
}

#[rstest]
fn a_recipe_rebuilds_the_setup_with_the_seats_that_name_a_companion(
    #[values(2, 3, 4)] seats: usize,
) {
    let (ids, cards) = deck_and_catalog();
    let catalog = catalog_of(&cards);
    let recipe = recipe(seats, &ids, &catalog);
    let back: GameRecipe = serde_json::from_str(&serde_json::to_string(&recipe).unwrap()).unwrap();
    assert_eq!(back, recipe);

    let setup = back.build_with(&catalog).unwrap();
    for (seat, player) in setup.players.iter().enumerate() {
        let name = player.companion.as_ref().map(|card| card.name.as_str());
        assert_eq!(name, (seat % 2 == 0).then_some("Small Companion"), "{seat}");
        assert_eq!(
            player.library.len(),
            60,
            "the companion is no card of the deck"
        );
    }
    assert_eq!(
        recipe
            .oracle_ids()
            .iter()
            .filter(|id| *id == COMPANION)
            .count(),
        1,
        "the companion is one of the ids the catalog is asked for, once"
    );
}

#[test]
fn the_stored_shape_names_the_companion_and_a_seat_without_one_keeps_the_shape_it_had() {
    let (ids, cards) = deck_and_catalog();
    let recipe = recipe(2, &ids, &catalog_of(&cards));
    let stored = serde_json::to_value(&recipe).unwrap();
    assert_eq!(stored["decks"][0]["companion"], json!(COMPANION));
    assert!(stored["decks"][1].get("companion").is_none());
    // A recipe stored before the companion has no such key and reads as a deck without one.
    let old = json!({"kind": "oracle", "cards": ["a", "b"]});
    let deck: DeckRef = serde_json::from_value(old).unwrap();
    assert_eq!(deck, DeckRef::oracle(vec!["a".into(), "b".into()]));
}

#[test]
fn the_companion_changes_the_fingerprint_of_its_seat_only() {
    let (ids, cards) = deck_and_catalog();
    let catalog = catalog_of(&cards);
    let with = recipe(2, &ids, &catalog);
    let without = GameRecipe::normal(7, [oracle(&ids, None), oracle(&ids, None)])
        .with_fingerprints_from(&catalog)
        .unwrap();
    assert_ne!(with.fingerprints[0], without.fingerprints[0]);
    assert_eq!(with.fingerprints[1], without.fingerprints[1]);
    let library: Vec<_> = ids.iter().map(|id| Arc::clone(&cards[id])).collect();
    assert_eq!(
        without.fingerprints[0],
        DeckFingerprint::of(&library).unwrap(),
        "a seat without a companion hashes as it did before"
    );
}

type Edit = fn(&mut GameRecipe, &mut BTreeMap<String, Arc<CardDefinition>>);

fn changed_card(_: &mut GameRecipe, cards: &mut BTreeMap<String, Arc<CardDefinition>>) {
    cards.insert(
        COMPANION.to_owned(),
        companion_with("Small Companion", small_permanents())
            .as_ref()
            .clone()
            .with_power_toughness(3, 3)
            .into(),
    );
}

fn swapped(recipe: &mut GameRecipe, cards: &mut BTreeMap<String, Arc<CardDefinition>>) {
    cards.insert(
        "companion-other".to_owned(),
        companion_with("Other Companion", small_permanents()),
    );
    if let DeckRef::Oracle { companion, .. } = &mut recipe.decks[0] {
        *companion = Some("companion-other".to_owned());
    }
}

fn dropped(recipe: &mut GameRecipe, _: &mut BTreeMap<String, Arc<CardDefinition>>) {
    if let DeckRef::Oracle { companion, .. } = &mut recipe.decks[0] {
        *companion = None;
    }
}

fn added(recipe: &mut GameRecipe, _: &mut BTreeMap<String, Arc<CardDefinition>>) {
    if let DeckRef::Oracle { companion, .. } = &mut recipe.decks[1] {
        *companion = Some(COMPANION.to_owned());
    }
}

#[rstest]
#[case::the_card_was_edited(changed_card as Edit, 0)]
#[case::another_card_is_named(swapped as Edit, 0)]
#[case::the_companion_is_gone_from_the_recipe(dropped as Edit, 0)]
#[case::a_seat_gained_one(added as Edit, 1)]
fn a_changed_companion_fails_the_rebuild_with_the_seat_named(
    #[case] edit: Edit,
    #[case] seat: usize,
    #[values(2, 3, 4)] seats: usize,
) {
    let (ids, mut cards) = deck_and_catalog();
    let mut recipe = recipe(seats, &ids, &catalog_of(&cards));
    edit(&mut recipe, &mut cards);
    match recipe.build_with(&catalog_of(&cards)).unwrap_err() {
        RecipeError::DeckChanged { seat: failed, .. } => assert_eq!(failed, seat),
        other => panic!("{other}"),
    }
}

#[test]
fn a_companion_the_catalog_lacks_is_named_with_its_seat() {
    let (ids, mut cards) = deck_and_catalog();
    let recipe = recipe(3, &ids, &catalog_of(&cards));
    cards.remove(COMPANION);
    match recipe.build_with(&catalog_of(&cards)).unwrap_err() {
        RecipeError::UnknownCards { seat, ids } => {
            assert_eq!(seat, 0);
            assert_eq!(ids, [COMPANION]);
        }
        other => panic!("{other}"),
    }
}

#[test]
fn a_deck_that_no_longer_meets_the_condition_is_refused_by_the_setup_check() {
    let (mut ids, mut cards) = deck_and_catalog();
    cards.insert("giant".to_owned(), creature("Giant", 5, &[], 5, 5));
    cards.insert("plains".to_owned(), land(mtg_core::color::Color::White));
    ids[0] = "giant".to_owned();
    let catalog = catalog_of(&cards);
    let recipe = recipe(2, &ids, &catalog);
    match recipe.build_with(&catalog).unwrap_err() {
        RecipeError::Setup(SetupError::InvalidCompanion { player, .. }) => {
            assert_eq!(player.index(), 0);
        }
        other => panic!("{other}"),
    }
    // The same cards at a seat that names none build.
    let plain = GameRecipe::normal(7, [oracle(&ids, None), oracle(&ids, None)])
        .with_fingerprints_from(&catalog)
        .unwrap();
    assert!(plain.build_with(&catalog).is_ok());
}
