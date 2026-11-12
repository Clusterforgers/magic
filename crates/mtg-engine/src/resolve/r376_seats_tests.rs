//! Goal: `Ref::Seated` names the next player still in the game on either side of a player at two to four seats (the
//! nearest opponent with `opponent`), skips a seat that has left, and keeps the seats when a rule reverses the turn order,
//! which runs `turn_order_from` and the next turn the other way round the table.

use mtg_cards::modify::Duration;
use mtg_cards::refs::{Ref, Side};
use mtg_core::ids::{EffectId, ObjectId, PlayerId};
use rstest::rstest;

use super::{Scope, Target};
use crate::fixtures::setup;
use crate::object::ObjectRef;
use crate::player::{LossReason, PlayerStatus};
use crate::rules::{Rule, RuleEffect};
use crate::state::GameState;

/// A game of `seats` seats with empty libraries and battlefields, seat 0 active.
fn game(seats: usize) -> GameState {
    let setup = setup(vec![vec![]; seats], vec![vec![]; seats]);
    GameState::from_setup(&setup).unwrap()
}

/// Every rule that reverses the turn order is one more reversal.
fn reverse(game: &mut GameState) {
    let effect = RuleEffect {
        id: EffectId(game.next_effect),
        rule: Rule::ReverseTurnOrder,
        duration: Duration::Forever,
        source: None,
        controller: PlayerId(0),
        turn: game.turn().number(),
    };
    game.next_effect += 1;
    game.rule_effects.push(effect);
}

/// The seats `Ref::Seated` names, read by seat `seat`.
fn seated(game: &GameState, seat: u64, side: Side, opponent: bool, of: Ref) -> Vec<u64> {
    let reference = Ref::Seated {
        of: Box::new(of),
        side,
        opponent,
    };
    let source = ObjectRef {
        id: ObjectId(0),
        counter: 0,
    };
    Scope::announcing(game, PlayerId(seat), source)
        .refs(&reference)
        .unwrap()
        .into_iter()
        .filter_map(Target::player)
        .map(|player| player.0)
        .collect()
}

#[rstest]
#[case::two_seats_left(2, 0, Side::Left, vec![1])]
#[case::two_seats_right(2, 0, Side::Right, vec![1])]
#[case::three_seats_left(3, 0, Side::Left, vec![1])]
#[case::three_seats_right(3, 0, Side::Right, vec![2])]
#[case::four_seats_left_of_the_last(4, 3, Side::Left, vec![0])]
#[case::four_seats_right_of_the_first(4, 0, Side::Right, vec![3])]
#[case::four_seats_right_of_seat_two(4, 2, Side::Right, vec![1])]
fn the_neighbour_on_each_side(
    #[case] seats: usize,
    #[case] seat: u64,
    #[case] side: Side,
    #[case] expected: Vec<u64>,
) {
    let game = game(seats);
    assert_eq!(seated(&game, seat, side, false, Ref::You), expected);
}

#[rstest]
#[case::left(Side::Left, vec![2])]
#[case::right(Side::Right, vec![3])]
fn a_seat_that_has_left_the_game_is_passed_over(#[case] side: Side, #[case] expected: Vec<u64>) {
    let mut game = game(4);
    game.players[1].status = PlayerStatus::Lost(LossReason::Conceded);
    assert_eq!(seated(&game, 0, side, false, Ref::You), expected);
}

#[test]
fn each_player_names_their_own_neighbour_each_once() {
    let game = game(3);
    assert_eq!(
        seated(&game, 0, Side::Left, false, Ref::EachPlayer),
        vec![1, 2, 0]
    );
    assert_eq!(
        seated(&game, 0, Side::Right, false, Ref::Opponents),
        vec![0, 1]
    );
}

#[test]
fn the_opponent_to_your_left_is_the_nearest_opponent() {
    let game = game(4);
    assert_eq!(seated(&game, 0, Side::Left, true, Ref::You), vec![1]);
}

#[rstest]
#[case::three_seats(3, vec![0, 2, 1])]
#[case::four_seats(4, vec![0, 3, 2, 1])]
fn a_reversed_turn_order_runs_right_and_keeps_the_seats(
    #[case] seats: usize,
    #[case] order: Vec<u64>,
) {
    let mut game = game(seats);
    reverse(&mut game);
    let turns: Vec<u64> = game
        .turn_order_from(PlayerId(0))
        .into_iter()
        .map(|p| p.0)
        .collect();
    assert_eq!(turns, order);
    assert_eq!(game.next_alive_after(PlayerId(0)), Some(PlayerId(order[1])));
    assert_eq!(seated(&game, 0, Side::Left, false, Ref::You), vec![1]);
}

#[rstest]
#[case::once(1, vec![0, 3, 2, 1])]
#[case::twice(2, vec![0, 1, 2, 3])]
#[case::three_times(3, vec![0, 3, 2, 1])]
fn two_reversals_cancel_out(#[case] times: usize, #[case] order: Vec<u64>) {
    let mut game = game(4);
    for _ in 0..times {
        reverse(&mut game);
    }
    let turns: Vec<u64> = game
        .turn_order_from(PlayerId(0))
        .into_iter()
        .map(|p| p.0)
        .collect();
    assert_eq!(turns, order);
}
