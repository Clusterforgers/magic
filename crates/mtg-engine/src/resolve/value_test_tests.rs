//! Goal: `Pred::Test` compares two amounts of the object it judges (`Ref::Candidate`), counts the objects for which it holds, and reads no candidate outside a test.

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::value::Amount;
use mtg_core::card::CardType;
use mtg_core::ids::PlayerId;
use rstest::rstest;

use super::Scope;
use crate::fixtures::setup;
use crate::state::GameState;

fn creature(name: &str, power: i32, toughness: i32) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Creature)
            .with_power_toughness(power, toughness),
    )
}

/// Seat 0 controls a 1/3, a 2/2 and a 4/2; seat 1 controls a 5/1.
fn game() -> GameState {
    let setup = setup(
        vec![vec![], vec![]],
        vec![
            vec![
                creature("Wall", 1, 3),
                creature("Bear", 2, 2),
                creature("Brute", 4, 2),
            ],
            vec![creature("Rival", 5, 1)],
        ],
    );
    GameState::from_setup(&setup).unwrap()
}

fn value(json: &str) -> Result<i64, crate::resolve::ResolveError> {
    let game = game();
    let source = game.objects.values().next().unwrap().reference();
    let amount: Amount = serde_json::from_str(json).unwrap();
    Scope::announcing(&game, PlayerId(0), source).amount(&amount)
}

fn counting(op: &str, left: &str, right: &str, who: &str) -> String {
    format!(
        r#"{{"Count":[{{"Type":"Creature"}},{{"ControlledBy":"{who}"}},{{"Test":{{
            "left":{{"Stat":{{"of":"Candidate","stat":"{left}"}}}},"op":"{op}",
            "right":{{"Stat":{{"of":"Candidate","stat":"{right}"}}}}}}}}]}}"#
    )
}

#[rstest]
#[case::power_below_toughness("Lt", "Power", "Toughness", "You", 1)]
#[case::power_equals_toughness("Eq", "Power", "Toughness", "You", 1)]
#[case::power_above_toughness("Gt", "Power", "Toughness", "You", 1)]
#[case::not_equal("Ne", "Power", "Toughness", "You", 2)]
#[case::the_other_seat("Gt", "Power", "Toughness", "Opponents", 1)]
#[case::unchanged_power_is_base_power("Gt", "Power", "BasePower", "You", 0)]
#[case::unchanged_power_equals_base_power("Eq", "Power", "BasePower", "You", 3)]
fn a_test_compares_two_stats_of_each_object(
    #[case] op: &str,
    #[case] left: &str,
    #[case] right: &str,
    #[case] who: &str,
    #[case] expected: i64,
) {
    assert_eq!(value(&counting(op, left, right, who)).unwrap(), expected);
}

#[test]
fn a_candidate_outside_a_test_is_no_object() {
    let outside = r#"{"Stat":{"of":"Candidate","stat":"Power"}}"#;
    assert!(value(outside).is_err());
}
