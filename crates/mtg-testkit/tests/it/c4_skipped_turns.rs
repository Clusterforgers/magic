//! Goal: a player who would begin an extra turn while a rule makes them skip it (Stranglehold, Gerrard's Hourglass Pendant) loses that turn at tables of two, three and four seats, whoever added it and in whatever order the extra turns were added, the other extra turns and the regular turn order go on unchanged (CR 500.7), each skipped turn is logged, and the log replays to the state it was played to.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use common::pid;
use mtg_cards::refs::Ref;
use mtg_core::ids::PlayerId;
use mtg_engine::effects::Duration;
use mtg_engine::rules::Rule;
use mtg_engine::turn::Step;
use mtg_engine::{Event, replay};
use mtg_testkit::{Driver, Scenario, ScriptError, time_spell};
use rstest::rstest;

/// A turn that began: its distance from the start, its seat and whether it is extra.
type Turn = (u32, usize, bool);

/// The turn the scenarios start in.
const START: u32 = 4;

/// Answers passively until the turn `number` has begun; every turn that begins on the way is recorded as its number,
/// its player and whether it is extra.
fn play_until(
    driver: &mut Driver,
    number: u32,
    seen: &mut Vec<(u32, PlayerId, bool)>,
) -> Result<(), ScriptError> {
    for _ in 0..4_000 {
        let turn = driver.state().turn();
        if seen.last().map(|began| began.0) != Some(turn.number()) {
            seen.push((turn.number(), turn.active(), turn.is_extra()));
        }
        if turn.number() >= number && driver.engine().decision().is_some() {
            return Ok(());
        }
        driver.answer_passively()?;
    }
    Err(ScriptError::Stuck(format!("turn {number}")))
}

/// `active` casts a sorcery that gives `who` the extra turns, the seats in `skippers` then skip every extra turn, and the
/// game is played until the turn that follows the last turn expected; the turns that began are returned, the skipped
/// turns logged are counted.
fn played(
    seats: usize,
    active: usize,
    who: Ref,
    skippers: &[usize],
    turns: usize,
) -> Result<(Vec<Turn>, usize), ScriptError> {
    let walk = time_spell("Hourglass", who, 1, false);
    let scenario = Scenario::new(seats)
        .at(START, active, Step::Main1)
        .hand(active, [walk]);
    let setup = scenario.clone().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario)?;
    driver.add_rule(
        Rule::SkipExtraTurns(skippers.iter().map(|seat| pid(*seat)).collect()),
        Duration::Forever,
    )?;
    driver.cast("Hourglass")?;
    while !driver.state().stack().is_empty() {
        driver.pass()?;
    }
    let mut seen = Vec::new();
    play_until(
        &mut driver,
        START + u32::try_from(turns).unwrap(),
        &mut seen,
    )?;
    let replayed = replay(&setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.engine().state_hash());
    assert_eq!(replayed.extra_turns(), driver.state().extra_turns());
    let skipped = driver
        .engine()
        .events()
        .iter()
        .filter(|event| matches!(event, Event::ExtraTurnSkipped { .. }))
        .count();
    let began = seen
        .iter()
        .skip(1)
        .map(|(number, player, extra)| {
            let seat = (0..seats).find(|seat| pid(*seat) == *player).unwrap();
            (number - START, seat, *extra)
        })
        .collect();
    Ok((began, skipped))
}

#[rstest]
#[case::mine_skipped_in_two(2, 0, Ref::You, &[0], &[(1, 1, false)], 1)]
#[case::theirs_skipped_in_two(2, 0, Ref::Opponents, &[1], &[(1, 1, false)], 1)]
#[case::nobody_skips(3, 1, Ref::You, &[], &[(1, 1, true), (2, 2, false)], 0)]
#[case::the_skipper_is_not_the_taker(3, 0, Ref::You, &[1, 2], &[(1, 0, true), (2, 1, false)], 0)]
#[case::the_latest_added_skipped_the_earlier_taken(2, 0, Ref::EachPlayer, &[1], &[(1, 0, true), (2, 1, false)], 1)]
#[case::all_the_opponents_of_three(3, 0, Ref::Opponents, &[1, 2], &[(1, 1, false)], 2)]
#[case::one_of_four(4, 2, Ref::EachPlayer, &[3], &[(1, 1, true), (2, 0, true), (3, 2, true), (4, 3, false)], 1)]
#[case::everyone_skips_in_four(4, 2, Ref::EachPlayer, &[0, 1, 2, 3], &[(1, 3, false)], 4)]
fn a_skipped_extra_turn_is_not_taken_and_the_others_and_the_turn_order_go_on(
    #[case] seats: usize,
    #[case] active: usize,
    #[case] who: Ref,
    #[case] skippers: &[usize],
    #[case] expected: &[(u32, usize, bool)],
    #[case] skipped: usize,
) -> Result<(), ScriptError> {
    let turns = expected.len();
    let (began, logged) = played(seats, active, who, skippers, turns)?;
    assert_eq!(began, expected);
    assert_eq!(logged, skipped);
    Ok(())
}
