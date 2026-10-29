//! Goal: the printing a deck line chose is kept in the recipe beside the oracle ids and reaches the cards of the setup at their positions (library, commanders, companion) at two to four seats, while the fingerprints and the state hash stay what they are without printings, a recipe stored before printings reads and writes as it did, and a position with no printing leaves the identity without one.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeMap;
use std::sync::Arc;

use mtg_cards::{CardDefinition, CardIdentity};
use mtg_core::color::Color;
use mtg_engine::rng::GameRng;
use mtg_engine::state::GameState;
use mtg_server::{Catalog, DeckRef, GameRecipe};
use mtg_testkit::cards::{creature, forest};
use mtg_testkit::commander_cards::legend;
use mtg_testkit::companion_cards::{small_companion, small_deck};
use rstest::rstest;
use serde_json::json;

const COMPANION: &str = "companion-small";

/// `card` as the card `oracle_id` the server read, with no printing.
fn identified(card: &Arc<CardDefinition>, oracle_id: &str) -> Arc<CardDefinition> {
    let mut card = (**card).clone();
    card.identity = Some(CardIdentity {
        oracle_id: oracle_id.to_owned(),
        printing_id: None,
    });
    Arc::new(card)
}

fn printing_of(card: &CardDefinition) -> Option<&str> {
    card.identity.as_ref()?.printing_id.as_deref()
}

fn hash(recipe: &GameRecipe, catalog: &Catalog) -> u64 {
    let setup = recipe.build_with(catalog).unwrap();
    GameState::from_setup(&setup).unwrap().state_hash()
}

/// The 60 cards of a Normal deck named by their card names, a companion, and the catalog that has them.
fn normal() -> (Vec<String>, Catalog) {
    let cards = small_deck(&mut GameRng::substream(9, 3));
    let mut known = BTreeMap::new();
    let ids = cards
        .iter()
        .map(|card| {
            known
                .entry(card.name.clone())
                .or_insert_with(|| identified(card, &card.name));
            card.name.clone()
        })
        .collect();
    known.insert(
        COMPANION.to_owned(),
        identified(&small_companion(), COMPANION),
    );
    (ids, Catalog::new(known))
}

/// A printing for every other position of `count` cards, and none for the rest.
fn alternate(count: usize) -> Vec<Option<String>> {
    (0..count)
        .map(|at| (at % 2 == 0).then(|| format!("print-{at}")))
        .collect()
}

#[rstest]
fn each_position_of_the_library_and_the_companion_takes_the_printing_of_its_line(
    #[values(2, 3, 4)] seats: usize,
) {
    let (ids, catalog) = normal();
    let mut printings = alternate(ids.len());
    printings.push(Some("print-companion".to_owned()));
    let deck = DeckRef::Oracle {
        cards: ids.clone(),
        commanders: Vec::new(),
        companion: Some(COMPANION.to_owned()),
        printings: printings.clone(),
    };
    let recipe = GameRecipe::normal(7, vec![deck; seats]);
    let setup = recipe.build_with(&catalog).unwrap();
    assert_eq!(setup.players.len(), seats);
    for player in &setup.players {
        assert_eq!(player.library.len(), ids.len());
        for (at, card) in player.library.iter().enumerate() {
            assert_eq!(printing_of(card), printings[at].as_deref(), "card {at}");
            assert_eq!(card.identity.as_ref().unwrap().oracle_id, ids[at]);
        }
        let companion = player.companion.as_ref().unwrap();
        assert_eq!(printing_of(companion), Some("print-companion"));
        assert_eq!(companion.identity.as_ref().unwrap().oracle_id, COMPANION);
    }
}

#[rstest]
fn printings_change_neither_the_fingerprints_nor_the_game(#[values(2, 3, 4)] seats: usize) {
    let (ids, catalog) = normal();
    let deck = |printings| DeckRef::Oracle {
        cards: ids.clone(),
        commanders: Vec::new(),
        companion: Some(COMPANION.to_owned()),
        printings,
    };
    let plain = GameRecipe::normal(7, vec![deck(Vec::new()); seats])
        .with_fingerprints_from(&catalog)
        .unwrap();
    let printed = GameRecipe::normal(7, vec![deck(alternate(61)); seats])
        .with_fingerprints_from(&catalog)
        .unwrap();
    assert_eq!(printed.fingerprints, plain.fingerprints);
    assert_eq!(hash(&printed, &catalog), hash(&plain, &catalog));
    // The fingerprint check passes for the recipe that carries printings.
    assert!(printed.build_with(&catalog).is_ok());
}

#[rstest]
fn a_commander_takes_the_printing_after_the_library(#[values(2, 3, 4)] seats: usize) {
    let mut cards = vec![("forest".to_owned(), identified(&forest(), "forest"))];
    for at in 60..99 {
        let bear = creature(&format!("Bear {at}"), 1, &[Color::Green], 2, 2);
        cards.push((
            format!("bear-{at}"),
            identified(&bear, &format!("bear-{at}")),
        ));
    }
    let chief = legend("Chief", 3, &[Color::Green], 3, 3);
    cards.push(("chief".to_owned(), identified(&chief, "chief")));
    let catalog = Catalog::new(cards);
    let mut ids = vec!["forest".to_owned(); 60];
    ids.extend((60..99).map(|at| format!("bear-{at}")));
    let mut printings = vec![None; 99];
    printings[0] = Some("first-forest".to_owned());
    printings[59] = Some("last-forest".to_owned());
    printings[98] = Some("last-bear".to_owned());
    printings.push(Some("the-chief".to_owned()));
    let deck = DeckRef::Oracle {
        cards: ids,
        commanders: vec!["chief".to_owned()],
        companion: None,
        printings,
    };
    let setup = GameRecipe::commander(3, vec![deck; seats])
        .build_with(&catalog)
        .unwrap();
    for player in &setup.players {
        let library: Vec<Option<&str>> = player.library.iter().map(|c| printing_of(c)).collect();
        assert_eq!(library.iter().flatten().count(), 3);
        assert_eq!(library[0], Some("first-forest"));
        assert_eq!(library[59], Some("last-forest"));
        assert_eq!(library[98], Some("last-bear"));
        assert_eq!(player.commanders.len(), 1);
        assert_eq!(printing_of(&player.commanders[0]), Some("the-chief"));
        assert_eq!(
            player.commanders[0].identity.as_ref().unwrap().oracle_id,
            "chief"
        );
        // Copies in one printing share a definition; a copy in another does not.
        assert!(Arc::ptr_eq(&player.library[1], &player.library[2]));
        assert!(!Arc::ptr_eq(&player.library[0], &player.library[1]));
    }
}

#[rstest]
#[case::none(vec![], json!({"kind": "oracle", "cards": ["a"], "commanders": ["c"], "companion": "p"}))]
#[case::some(
    vec![Some("x".to_owned()), None, Some("z".to_owned())],
    json!({"kind": "oracle", "cards": ["a"], "commanders": ["c"], "companion": "p", "printings": ["x", null, "z"]})
)]
fn the_stored_form_names_printings_only_when_there_are_some(
    #[case] printings: Vec<Option<String>>,
    #[case] stored: serde_json::Value,
) {
    let deck = DeckRef::Oracle {
        cards: vec!["a".to_owned()],
        commanders: vec!["c".to_owned()],
        companion: Some("p".to_owned()),
        printings,
    };
    assert_eq!(serde_json::to_value(&deck).unwrap(), stored);
    let back: DeckRef = serde_json::from_value(stored).unwrap();
    assert_eq!(back, deck);
}
