//! Goal: the l291 patterns trigger on exactly their events: a die result in the list triggers once per such die with the result as its amount, a roll to visit carries the number rolled, and another number, another player's die or a roll without results in the list never match.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_core::ids::PlayerId;
use rstest::rstest;

use super::detect_events::{FOE, ME, board, fires};
use crate::choose::Target;
use crate::events::Event;

/// Dice rolled in one step by the players of `rolls` as (player, result) on d6.
fn l291_rolls(rolls: &[(PlayerId, u32)]) -> Vec<Event> {
    rolls
        .iter()
        .map(|(player, result)| Event::DieRolled {
            player: *player,
            sides: 6,
            result: *result,
        })
        .collect()
}

#[rstest]
#[case::a_one(r#"{"DiceRolled":{"who":"You","results":[1]}}"#, &[(ME, 1)], &[1])]
#[case::another_number(r#"{"DiceRolled":{"who":"You","results":[1]}}"#, &[(ME, 2)], &[])]
#[case::their_one(r#"{"DiceRolled":{"who":"You","results":[1]}}"#, &[(FOE, 1)], &[])]
#[case::each_die_of_the_list(
    r#"{"DiceRolled":{"who":"You","results":[1,2]}}"#,
    &[(ME, 2), (ME, 5), (ME, 1)],
    &[2, 1]
)]
#[case::an_opponent_rolls(r#"{"DiceRolled":{"who":"Opponents","results":[6]}}"#, &[(ME, 6), (FOE, 6)], &[6])]
fn l291_a_listed_result_triggers_once_for_each_die_with_its_number(
    #[case] when: &str,
    #[case] rolls: &[(PlayerId, u32)],
    #[case] amounts: &[u32],
) {
    let found = fires(&board(when), &l291_rolls(rolls));
    let got: Vec<u32> = found.iter().map(|t| t.cause.amount).collect();
    assert_eq!(got, amounts);
    for triggered in &found {
        assert!(matches!(triggered.cause.subjects[..], [Target::Player(_)]));
    }
}

#[rstest]
#[case::a_six(6)]
#[case::a_one(1)]
fn l291_a_roll_to_visit_carries_the_number_rolled(#[case] result: u32) {
    let found = fires(
        &board(r#"{"RolledToVisit":{"who":"You"}}"#),
        &[Event::RolledToVisit {
            player: ME,
            result,
            visited: Vec::new(),
        }],
    );
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].cause.amount, result);
}
