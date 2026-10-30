//! Goal: a tap with a name binds exactly the permanents it tapped, as the objects they are, and never one that was tapped before.
use mtg_cards::effect::Effect;
use mtg_core::ids::PlayerId;
use rstest::rstest;

use crate::fixtures::{creature, setup};
use crate::resolve::{Cx, Resolving, Run, Scope};
use crate::state::GameState;

/// Seat 0 controls the Source. Seat 1 controls a Bear, a Cat and a tapped Ogre.
fn game() -> GameState {
    let mut game = setup(
        vec![vec![], vec![]],
        vec![
            vec![creature("Source", 1, 1)],
            vec![
                creature("Bear", 2, 2),
                creature("Cat", 1, 1),
                creature("Ogre", 3, 4),
            ],
        ],
    );
    game.players[1].battlefield[2].tapped = true;
    GameState::from_setup(&game).unwrap()
}

#[rstest]
#[case::each_creature_of_the_opponent(
    r#"{"Tap":{"what":{"Each":[{"Type":"Creature"},{"ControlledBy":"Opponents"}]},"as":"tapped"}}"#,
    &["Bear", "Cat"]
)]
#[case::only_the_tapped_one(
    r#"{"Tap":{"what":{"Each":[{"Named":"Ogre"}]},"as":"tapped"}}"#,
    &[]
)]
#[case::the_source(r#"{"Tap":{"what":"This","as":"tapped"}}"#, &["Source"])]
fn a_named_tap_binds_what_it_tapped(#[case] json: &str, #[case] expected: &[&str]) {
    let game = game();
    let effect: Effect = serde_json::from_str(json).unwrap();
    let source = game
        .objects()
        .find(|o| o.card().name == "Source")
        .map(crate::object::GameObject::reference)
        .unwrap();
    let resolving = Resolving::begin(source.id);
    let cx = Cx {
        scope: Scope::announcing(&game, PlayerId(0), source),
        resolving: &resolving,
    };
    let Run::Done { bound, .. } = super::tap::run(&cx, &effect).unwrap() else {
        panic!("a tap needs no answer");
    };
    let [(name, targets)] = &bound[..] else {
        panic!("one name was expected: {bound:?}");
    };
    assert_eq!(name, "tapped");
    let mut names: Vec<_> = targets
        .iter()
        .copied()
        .filter_map(crate::choose::Target::object)
        .map(|o| game.object(o.id).unwrap().card().name.clone())
        .collect();
    names.sort();
    assert_eq!(names, expected);
}
