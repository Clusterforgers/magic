//! Goal: `Ref::PlayersWhere` names the players of its group for whom the condition holds with each taken as "you", at three seats, names nobody when it holds for none, keeps `Ref::Asker` the player whose ability asks, and `Condition::Among` tells the player tested from the players a reference names.

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::value::Amount;
use mtg_core::card::CardType;
use mtg_core::ids::PlayerId;
use rstest::rstest;

use super::Scope;
use crate::fixtures::setup;
use crate::state::GameState;

fn creature(name: &str) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Creature)
            .with_power_toughness(2, 2),
    )
}

/// Three seats, seat 0 active: seat 0 controls two creatures, seat 1 one, seat 2 none; seat 2 is at 7 life.
fn game() -> GameState {
    let setup = setup(
        vec![vec![], vec![], vec![]],
        vec![
            vec![creature("Bear"), creature("Ogre")],
            vec![creature("Rival")],
            vec![],
        ],
    );
    let mut game = GameState::from_setup(&setup).unwrap();
    game.players[2].life = 7;
    game
}

/// The number of players `{"PlayersWhere": {"who": who, "test": test}}` names, read by seat `seat`.
fn count(seat: u64, who: &str, test: &str) -> i64 {
    let game = game();
    let source = game.objects.values().next().unwrap().reference();
    let json = format!(r#"{{"Players":{{"PlayersWhere":{{"who":"{who}","test":{test}}}}}}}"#);
    let amount: Amount = serde_json::from_str(&json).unwrap();
    Scope::announcing(&game, PlayerId(seat), source)
        .amount(&amount)
        .unwrap()
}

const HAS_A_CREATURE: &str = r#"{"Compare":{"left":{"Count":[{"Type":"Creature"},{"ControlledBy":"You"}]},"op":"Ge","right":1}}"#;
const TWO_CREATURES: &str = r#"{"Compare":{"left":{"Count":[{"Type":"Creature"},{"ControlledBy":"You"}]},"op":"Ge","right":2}}"#;
const LOW_LIFE: &str = r#"{"Compare":{"left":{"Life":"You"},"op":"Le","right":10}}"#;

#[rstest]
#[case::players_with_a_creature(0, "EachPlayer", HAS_A_CREATURE, 2)]
#[case::opponents_with_a_creature(0, "Opponents", HAS_A_CREATURE, 1)]
#[case::opponents_of_seat_one(1, "Opponents", HAS_A_CREATURE, 1)]
#[case::two_creatures(0, "EachPlayer", TWO_CREATURES, 1)]
#[case::two_creatures_among_opponents(0, "Opponents", TWO_CREATURES, 0)]
#[case::low_life(1, "Opponents", LOW_LIFE, 1)]
#[case::not_the_active_player(0, "EachPlayer", r#"{"Not":"YourTurn"}"#, 2)]
#[case::the_active_player(1, "Opponents", r#""YourTurn""#, 1)]
#[case::the_active_player_is_not_their_own_opponent(0, "Opponents", r#""YourTurn""#, 0)]
fn the_players_of_the_group_for_whom_it_holds(
    #[case] seat: u64,
    #[case] who: &str,
    #[case] test: &str,
    #[case] expected: i64,
) {
    assert_eq!(count(seat, who, test), expected);
}

const MORE_THAN_ASKER: &str = r#"{"Compare":{"left":{"Count":[{"Type":"Creature"},{"ControlledBy":"You"}]},"op":"Gt","right":{"Count":[{"Type":"Creature"},{"ControlledBy":"Asker"}]}}}"#;
const FEWER_THAN_ASKER: &str = r#"{"Compare":{"left":{"Count":[{"Type":"Creature"},{"ControlledBy":"You"}]},"op":"Lt","right":{"Count":[{"Type":"Creature"},{"ControlledBy":"Asker"}]}}}"#;
const TWO_MORE_THAN_ASKER: &str = r#"{"Compare":{"left":{"Count":[{"Type":"Creature"},{"ControlledBy":"You"}]},"op":"Ge","right":{"Sum":[{"Count":[{"Type":"Creature"},{"ControlledBy":"Asker"}]},2]}}}"#;

#[rstest]
#[case::more_than_the_last_seat(2, "Opponents", MORE_THAN_ASKER, 2)]
#[case::more_than_the_middle_seat(1, "Opponents", MORE_THAN_ASKER, 1)]
#[case::nobody_has_more_than_the_first_seat(0, "EachPlayer", MORE_THAN_ASKER, 0)]
#[case::fewer_than_the_first_seat(0, "Opponents", FEWER_THAN_ASKER, 2)]
#[case::fewer_than_the_middle_seat(1, "Opponents", FEWER_THAN_ASKER, 1)]
#[case::two_more_than_the_last_seat(2, "EachPlayer", TWO_MORE_THAN_ASKER, 1)]
#[case::two_more_than_the_middle_seat(1, "EachPlayer", TWO_MORE_THAN_ASKER, 0)]
#[case::every_player_but_the_asker(1, "EachPlayer", r#"{"Not":{"Among":"Asker"}}"#, 2)]
#[case::the_opponents_but_the_active_player(
    1,
    "Opponents",
    r#"{"Not":{"Among":"ActivePlayer"}}"#,
    1
)]
#[case::the_active_player_among_them(2, "EachPlayer", r#"{"Among":"ActivePlayer"}"#, 1)]
#[case::nobody_among_nobody(0, "EachPlayer", r#"{"Among":"Monarch"}"#, 0)]
fn the_asker_stays_the_player_whose_ability_asks(
    #[case] seat: u64,
    #[case] who: &str,
    #[case] test: &str,
    #[case] expected: i64,
) {
    assert_eq!(count(seat, who, test), expected);
}

#[rstest]
#[case::first_seat(0, 2)]
#[case::middle_seat(1, 1)]
#[case::last_seat(2, 0)]
fn the_asker_outside_a_test_is_the_controller(#[case] seat: u64, #[case] expected: i64) {
    let game = game();
    let source = game.objects.values().next().unwrap().reference();
    let amount: Amount =
        serde_json::from_str(r#"{"Count":[{"Type":"Creature"},{"ControlledBy":"Asker"}]}"#)
            .unwrap();
    let read = Scope::announcing(&game, PlayerId(seat), source)
        .amount(&amount)
        .unwrap();
    assert_eq!(read, expected);
}
