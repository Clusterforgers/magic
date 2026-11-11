//! Goal: a dice pattern with results triggers once for each die whose result it names, for the players it names only, and hands the ability the result as its amount.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_core::ids::PlayerId;
use rstest::rstest;

use super::detect_events::{FOE, ME, board, fires};
use crate::events::Event;

/// Dice rolled in one step by the players of `rolls` as (player, sides, result).
fn rolls(rolls: &[(PlayerId, u32, u32)]) -> Vec<Event> {
    rolls
        .iter()
        .map(|(player, sides, result)| Event::DieRolled {
            player: *player,
            sides: *sides,
            result: *result,
        })
        .collect()
}

#[rstest]
#[case::a_one(r#"{"DiceRolled":{"who":"You","results":[1]}}"#, &[(ME, 6, 1)], &[1])]
#[case::a_two_is_not_a_one(r#"{"DiceRolled":{"who":"You","results":[1]}}"#, &[(ME, 6, 2)], &[])]
#[case::one_or_two_each_die(
    r#"{"DiceRolled":{"who":"You","results":[1,2]}}"#,
    &[(ME, 6, 2), (ME, 6, 5), (ME, 6, 1)],
    &[2, 1]
)]
#[case::their_one_not_mine(r#"{"DiceRolled":{"who":"You","results":[1]}}"#, &[(FOE, 6, 1)], &[])]
#[case::anyone_rolls_a_six(r#"{"DiceRolled":{"results":[6]}}"#, &[(FOE, 6, 6), (ME, 6, 6)], &[6, 6])]
#[case::natural_twenty(
    r#"{"DiceRolled":{"who":"You","results":[20],"natural":true}}"#,
    &[(ME, 20, 20), (ME, 20, 19)],
    &[20]
)]
#[case::without_results_once_a_roll(r#"{"DiceRolled":{"who":"You"}}"#, &[(ME, 6, 1), (ME, 6, 2)], &[0])]
fn r289_a_result_triggers_for_each_die_that_shows_it(
    #[case] when: &str,
    #[case] dice: &[(PlayerId, u32, u32)],
    #[case] amounts: &[u32],
) {
    let found = fires(&board(when), &rolls(dice));
    let got: Vec<u32> = found.iter().map(|t| t.cause.amount).collect();
    assert_eq!(got, amounts);
}
