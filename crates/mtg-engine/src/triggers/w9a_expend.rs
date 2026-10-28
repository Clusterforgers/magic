//! Goal: expend N triggers once, for the cast whose mana takes the player's turn total of mana spent on spells from below N to N or more, for the players the pattern names; casts in one batch count in order, and a free cast expends nothing.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_core::ids::PlayerId;
use mtg_core::mana::ManaType;
use rstest::rstest;

use super::detect_events::{FOE, ME, board, fired, id};
use crate::choose::Choices;
use crate::events::Event;
use crate::state::{GameState, Tally};

/// A cast of `name` by `player` that spent `mana` red mana.
fn w9a_cast(state: &GameState, player: PlayerId, name: &str, mana: usize) -> Event {
    let choices = Choices {
        spent: (mana > 0).then(|| Box::new(vec![ManaType::Red; mana])),
        ..Choices::default()
    };
    Event::SpellCast {
        player,
        object: id(state, name),
        choices,
    }
}

#[rstest]
#[case::reaching_four_at_once(r#"{"Expend":{"who":"You","n":4}}"#, 0, 4, 1)]
#[case::passing_four(r#"{"Expend":{"who":"You","n":4}}"#, 3, 2, 1)]
#[case::below_four(r#"{"Expend":{"who":"You","n":4}}"#, 1, 2, 0)]
#[case::four_already_spent(r#"{"Expend":{"who":"You","n":4}}"#, 4, 3, 0)]
#[case::a_free_cast(r#"{"Expend":{"who":"You","n":4}}"#, 4, 0, 0)]
#[case::eight_from_five(r#"{"Expend":{"who":"You","n":8}}"#, 5, 3, 1)]
#[case::six_skipped_past(r#"{"Expend":{"who":"You","n":6}}"#, 2, 7, 1)]
fn expend_triggers_as_the_turn_total_reaches_n(
    #[case] when: &str,
    #[case] earlier: usize,
    #[case] now: usize,
    #[case] expected: usize,
) {
    let mut pre = board(when);
    pre.tally_by(ME, Tally::W9aSpellMana, u32::try_from(earlier).unwrap());
    let cast = w9a_cast(&pre, ME, "Spell", now);
    assert_eq!(fired(&pre, &[cast]), expected);
}

#[rstest]
#[case::mine(r#"{"Expend":{"who":"You","n":2}}"#, ME, 1)]
#[case::not_theirs(r#"{"Expend":{"who":"You","n":2}}"#, FOE, 0)]
#[case::an_opponents(r#"{"Expend":{"who":"Opponents","n":2}}"#, FOE, 1)]
#[case::anyones(r#"{"Expend":{"n":2}}"#, FOE, 1)]
fn expend_names_the_players_of_the_pattern(
    #[case] when: &str,
    #[case] player: PlayerId,
    #[case] expected: usize,
) {
    let pre = board(when);
    let name = if player == ME { "Spell" } else { "Their Spell" };
    let cast = w9a_cast(&pre, player, name, 3);
    assert_eq!(fired(&pre, &[cast]), expected);
}

#[rstest]
#[case::the_first_reaches_it(2, 2, 1)]
#[case::the_second_reaches_it(1, 2, 1)]
#[case::neither_reaches_it(1, 1, 0)]
fn two_casts_in_one_batch_count_in_order_and_trigger_once(
    #[case] first: usize,
    #[case] second: usize,
    #[case] expected: usize,
) {
    let pre = board(r#"{"Expend":{"who":"You","n":3}}"#);
    let events = [
        w9a_cast(&pre, ME, "Spell", first),
        w9a_cast(&pre, ME, "Their Spell", second),
    ];
    assert_eq!(fired(&pre, &events), expected);
}
