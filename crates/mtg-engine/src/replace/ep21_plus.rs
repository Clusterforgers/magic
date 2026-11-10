//! Goal: `ReplaceBy::Plus` creates the tokens and the tokens its effects make as one event: "that many" counts the tokens
//! of the event, a fixed count adds that many once, a kind filter leaves other tokens alone and another player's
//! tokens get nothing, at tables of two to four seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_core::ids::{ObjectId, PlayerId};
use rstest::rstest;

use super::{Processed, process};
use crate::events::Event;
use crate::fixtures::{creature, state};
use crate::state::GameState;
use crate::tokens::TokenCreated;

const SQUIRRELS: &str = r#"{"Replacement":{"when":{"TokenCreation":{}},"by":{"Plus":[{"CreateToken":{"token":{"Custom":{"name":"Squirrel","colors":["Green"],"types":["Creature"],"subtypes":["Squirrel"],"power":1,"toughness":1}},"count":"Triggering"}}]}}}"#;
const FOOD: &str = r#"{"Replacement":{"when":{"TokenCreation":{}},"by":{"Plus":[{"CreateToken":{"token":{"Named":"food"}}}]}}}"#;
const TREASURE: &str = r#"{"Replacement":{"when":{"TokenCreation":{"what":[{"Subtype":"Treasure"}]}},"by":{"Plus":[{"CreateToken":{"token":{"Named":"treasure"}}}]}}}"#;

/// Seat 0 controls one Source per rule; the other seats control a creature each.
fn game(seats: usize, rules: &[&str]) -> GameState {
    let mut battlefields: Vec<_> = (0..seats)
        .map(|n| vec![creature(&format!("C{n}"), 1, 1)])
        .collect();
    for rule in rules {
        let ability: Ability = serde_json::from_str(rule).unwrap();
        let source = CardDefinition::new("Source")
            .with_type(CardType::Enchantment)
            .with_rules(ability);
        battlefields[0].push(Arc::new(source));
    }
    state(vec![vec![]; seats], battlefields)
}

fn token(game: &GameState, id: u64, controller: u64, name: &str, subtype: &str) -> Event {
    let definition = CardDefinition::new(name)
        .with_type(CardType::Artifact)
        .with_subtype(subtype);
    Event::TokenCreated(TokenCreated {
        object: ObjectId(game.next_object + id),
        controller: PlayerId(controller),
        name: name.into(),
        tapped: false,
        attacking: None,
        blocking: None,
        definition: Arc::new(definition),
        copy_of: None,
    })
}

/// The names of the tokens made, in order.
fn made(game: &GameState, batch: Vec<Event>) -> Vec<String> {
    let Processed::Events(out) = process(game, batch, &[]).unwrap() else {
        panic!("nothing is asked");
    };
    let ids: Vec<ObjectId> = out
        .iter()
        .filter_map(|event| match event {
            Event::TokenCreated(token) => Some(token.object),
            _ => None,
        })
        .collect();
    let mut unique = ids.clone();
    unique.sort_unstable();
    unique.dedup();
    assert_eq!(unique.len(), ids.len(), "every token is a new object");
    out.iter()
        .filter_map(|event| match event {
            Event::TokenCreated(token) => Some(token.name.clone()),
            _ => None,
        })
        .collect()
}

fn count(names: &[String], name: &str) -> usize {
    names.iter().filter(|found| *found == name).count()
}

#[rstest]
fn that_many_squirrels_join_the_tokens(
    #[values(2, 3, 4)] seats: usize,
    #[values(1, 3)] tokens: u64,
) {
    let game = game(seats, &[SQUIRRELS]);
    let batch = (0..tokens)
        .map(|id| token(&game, id, 0, "Clue", "Clue"))
        .collect();
    let names = made(&game, batch);
    assert_eq!(
        count(&names, "Clue"),
        usize::try_from(tokens).unwrap(),
        "{names:?}"
    );
    assert_eq!(
        count(&names, "Squirrel"),
        usize::try_from(tokens).unwrap(),
        "{names:?}"
    );
}

#[rstest]
#[case::one(1)]
#[case::three(3)]
fn a_fixed_count_adds_one_food_to_the_event(#[case] tokens: u64) {
    let game = game(2, &[FOOD]);
    let batch = (0..tokens)
        .map(|id| token(&game, id, 0, "Clue", "Clue"))
        .collect();
    let names = made(&game, batch);
    assert_eq!(count(&names, "Clue"), usize::try_from(tokens).unwrap());
    assert_eq!(count(&names, "Food"), 1, "{names:?}");
}

#[rstest]
#[case::treasure("Treasure", 2)]
#[case::other_kind("Clue", 0)]
fn a_kind_filter_adds_only_for_its_tokens(#[case] made_kind: &str, #[case] treasures: usize) {
    let game = game(3, &[TREASURE]);
    let names = made(&game, vec![token(&game, 0, 0, made_kind, made_kind)]);
    assert_eq!(count(&names, "Treasure"), treasures, "{names:?}");
    assert_eq!(names.len(), 1 + usize::from(treasures > 0), "{names:?}");
}

#[rstest]
fn another_players_tokens_get_nothing(#[values(2, 4)] seats: usize) {
    let game = game(seats, &[SQUIRRELS, FOOD]);
    let names = made(&game, vec![token(&game, 0, 1, "Clue", "Clue")]);
    assert_eq!(names, ["Clue"]);
}

const EACH: &str = r#"{"Replacement":{"when":{"TokenCreation":{"what":[{"AnyOf":[{"Subtype":"Clue"},{"Subtype":"Food"},{"Subtype":"Treasure"}]}]}},"by":{"Instead":[{"CreateToken":{"token":{"Named":"clue"},"count":"Triggering"}},{"CreateToken":{"token":{"Named":"food"},"count":"Triggering"}},{"CreateToken":{"token":{"Named":"treasure"},"count":"Triggering"}}]}}}"#;

/// A kind named by alternatives ("a Clue, Food, or Treasure token") covers each of them and no other token.
#[rstest]
#[case::two_treasures(&["Treasure", "Treasure"], 2, 0)]
#[case::a_clue_and_a_food(&["Clue", "Food"], 2, 0)]
#[case::a_map_stays(&["Map"], 0, 1)]
fn one_of_each_replaces_each_token_of_the_kinds(
    #[case] kinds: &[&str],
    #[case] each: usize,
    #[case] maps: usize,
) {
    let game = game(2, &[EACH]);
    let batch = kinds
        .iter()
        .enumerate()
        .map(|(id, kind)| token(&game, id as u64, 0, kind, kind))
        .collect();
    let names = made(&game, batch);
    for kind in ["Clue", "Food", "Treasure"] {
        assert_eq!(count(&names, kind), each, "{names:?}");
    }
    assert_eq!(count(&names, "Map"), maps, "{names:?}");
}
