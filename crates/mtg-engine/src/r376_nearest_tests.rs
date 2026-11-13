//! Goal: a rule that lets each player attack only the nearest opponent on a side keeps that one opponent and the
//! planeswalkers they control among the defenders at two to four seats, passes over a seat that has left, applies with
//! every other such rule (sides that disagree leave nobody at three seats or more), and names nothing without a side.

use mtg_cards::modify::Duration;
use mtg_core::ids::{EffectId, PlayerId};
use rstest::rstest;

use crate::combat::{AttackTarget, defenders_for};
use crate::fixtures::setup;
use crate::player::{LossReason, PlayerStatus};
use crate::rules::{Rule, RuleEffect, names_nothing};
use crate::state::GameState;

fn game(seats: usize) -> GameState {
    let setup = setup(vec![vec![]; seats], vec![vec![]; seats]);
    GameState::from_setup(&setup).unwrap()
}

/// Puts in force the rule for the nearest opponent on the left (`true`) or the right.
fn nearest(game: &mut GameState, left: bool) {
    let effect = RuleEffect {
        id: EffectId(game.next_effect),
        rule: Rule::AttackOnlyNearest(Some(left)),
        duration: Duration::Forever,
        source: None,
        controller: PlayerId(0),
        turn: game.turn().number(),
    };
    game.next_effect += 1;
    game.rule_effects.push(effect);
}

/// The players `player` may attack.
fn attacked(game: &GameState, player: u64) -> Vec<u64> {
    defenders_for(game, PlayerId(player))
        .into_iter()
        .filter_map(|target| match target {
            AttackTarget::Player(seat) => Some(seat.0),
            AttackTarget::Planeswalker(_) => None,
        })
        .collect()
}

#[rstest]
#[case::two_seats_left(2, 0, &[true], vec![1])]
#[case::two_seats_both_sides(2, 0, &[true, false], vec![1])]
#[case::three_seats_left(3, 0, &[true], vec![1])]
#[case::three_seats_right(3, 0, &[false], vec![2])]
#[case::four_seats_right_of_seat_two(4, 2, &[false], vec![1])]
#[case::four_seats_left_of_the_last(4, 3, &[true], vec![0])]
#[case::three_seats_disagreeing(3, 0, &[true, false], vec![])]
#[case::no_rule(4, 0, &[], vec![1, 2, 3])]
fn only_the_nearest_opponent_on_each_side_may_be_attacked(
    #[case] seats: usize,
    #[case] player: u64,
    #[case] sides: &[bool],
    #[case] expected: Vec<u64>,
) {
    let mut game = game(seats);
    for left in sides {
        nearest(&mut game, *left);
    }
    let mut found = attacked(&game, player);
    found.sort_unstable();
    assert_eq!(found, expected);
}

#[test]
fn a_seat_that_has_left_is_passed_over() {
    let mut game = game(4);
    game.players[1].status = PlayerStatus::Lost(LossReason::Conceded);
    nearest(&mut game, true);
    assert_eq!(attacked(&game, 0), vec![2]);
}

#[rstest]
#[case::no_side(Rule::AttackOnlyNearest(None), true)]
#[case::left(Rule::AttackOnlyNearest(Some(true)), false)]
fn a_rule_without_a_side_names_nothing(#[case] rule: Rule, #[case] nothing: bool) {
    assert_eq!(names_nothing(&rule), nothing);
}
