//! Goal: a mana ability that adds mana of one color in a number the game counts when it is offered ("X mana of any one color, where X is the number of creatures you control") is a source for each color with exactly that many mana and none while the count is zero, and one whose mana a condition replaces ("Add {G}. If you control four or more creatures, add {G}{G} instead.") adds the mana of the branch that holds now.
use std::sync::Arc;

use mtg_cards::CardDefinition;
use rstest::rstest;

use super::*;
use crate::fixtures::{creature, state};

const ME: PlayerId = PlayerId(0);

const BY_CREATURES: &str = r#"{"Activated":{"cost":["Tap"],"effects":[{"AddMana":{"mana":{"AnyColor":{"Count":[{"Type":"Creature"},{"ControlledBy":"You"}]}}}}]}}"#;
const BY_ITS_POWER: &str = r#"{"Activated":{"cost":["Tap"],"effects":[{"AddMana":{"mana":{"AnyColor":{"Stat":{"of":"This","stat":"Power"}}}}}]}}"#;
const LEAFKIN: &str = r#"{"Activated":{"cost":["Tap"],"effects":[{"AddMana":{"mana":{"Instead":{"base":{"Fixed":["Green"]},"when":{"Compare":{"left":{"Count":[{"Type":"Creature"},{"ControlledBy":"You"}]},"op":"Ge","right":4}},"instead":{"Fixed":["Green","Green"]}}}}}]}}"#;
const CARYATID: &str = r#"{"Activated":{"cost":["Tap"],"effects":[{"AddMana":{"mana":{"Instead":{"base":{"AnyColor":1},"when":{"Compare":{"left":{"Count":[{"Type":"Creature"},{"Compare":{"stat":"Power","op":"Ge","to":4}},{"ControlledBy":"You"}]},"op":"Ge","right":1}},"instead":{"AnyColor":2}}}}}]}}"#;

fn rock(json: &str) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new("Rock")
            .with_type(CardType::Artifact)
            .with_rules(serde_json::from_str(json).unwrap()),
    )
}

/// A game where seat 0 has the rock and `creatures` creatures of power `power`; seat 1 has a creature as well.
fn game(json: &str, creatures: usize, power: i32) -> GameState {
    let mut mine = vec![rock(json)];
    mine.extend((0..creatures).map(|n| creature(&format!("Elf {n}"), power, 1)));
    state(
        vec![vec![], vec![]],
        vec![mine, vec![creature("Foe", 2, 2)]],
    )
}

/// The options of the rock.
fn of_rock(game: &GameState) -> Vec<ManaOption> {
    let id = game.zone(Zone::Battlefield)[0];
    options(game, ME)
        .into_iter()
        .filter(|option| option.source.object == id)
        .collect()
}

#[rstest]
#[case::none(0)]
#[case::one(1)]
#[case::three(3)]
#[case::five(5)]
fn mana_of_one_color_is_as_many_as_the_creatures_counted_now(#[case] creatures: usize) {
    let found = of_rock(&game(BY_CREATURES, creatures, 1));
    if creatures == 0 {
        assert!(found.is_empty(), "a count of nothing is not offered");
        return;
    }
    assert_eq!(found.len(), 5, "one source for each color");
    for option in &found {
        assert_eq!(option.made().len(), creatures);
        assert!(option.made().iter().all(|mana| *mana == option.produces));
    }
}

#[test]
fn a_foes_creatures_are_not_counted() {
    let found = of_rock(&game(BY_CREATURES, 2, 1));
    assert!(found.iter().all(|option| option.made().len() == 2));
}

#[rstest]
#[case::three_creatures(3, 1)]
#[case::four_creatures(4, 2)]
#[case::six_creatures(6, 2)]
fn mana_is_doubled_while_four_creatures_are_controlled(
    #[case] creatures: usize,
    #[case] mana: usize,
) {
    let found = of_rock(&game(LEAFKIN, creatures, 1));
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].made().len(), mana);
    assert_eq!(found[0].produces, ManaType::Green);
}

#[rstest]
#[case::small_creatures(3, 1)]
#[case::one_big_creature(4, 2)]
fn two_mana_of_one_color_replace_one_while_a_creature_has_power_four(
    #[case] power: i32,
    #[case] mana: usize,
) {
    let found = of_rock(&game(CARYATID, 1, power));
    assert_eq!(found.len(), 5, "one source for each color");
    assert!(found.iter().all(|option| option.made().len() == mana));
}

#[test]
fn a_count_of_a_characteristic_of_the_source_is_read_for_the_source() {
    let sticks = Arc::new(
        CardDefinition::new("Stick")
            .with_type(CardType::Artifact)
            .with_rules(serde_json::from_str(BY_ITS_POWER).unwrap()),
    );
    let game = state(vec![vec![], vec![]], vec![vec![sticks], vec![]]);
    assert!(
        of_rock(&game).is_empty(),
        "an artifact has no power, so it adds nothing"
    );
}
