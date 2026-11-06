//! Goal: "skip your next turn" (a rule that a player skips a turn and ends with the turn it skips, CR 614.10) removes
//! exactly one turn of the player for each such rule, the turn order goes on unchanged, the rule is gone afterwards, each
//! rule used is logged as expired, and the log replays to the state it was played to, at tables of two to four seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use common::pid;
use mtg_cards::modify::Prohibited;
use mtg_engine::choose::Target;
use mtg_engine::effects::Duration;
use mtg_engine::rules::Rule;
use mtg_engine::turn::Step;
use mtg_engine::{Event, replay};
use mtg_testkit::{Driver, Scenario, ScriptError};
use rstest::rstest;

/// The turn the scenarios start in.
const START: u32 = 4;

/// Answers passively until `turns` more turns have begun; the seat of each is recorded.
fn play(driver: &mut Driver, seats: usize, turns: usize) -> Result<Vec<usize>, ScriptError> {
    let mut seen: Vec<(u32, usize)> = Vec::new();
    for _ in 0..4_000 {
        let turn = driver.state().turn();
        if seen.last().map(|began| began.0) != Some(turn.number()) {
            let seat = (0..seats).find(|seat| pid(*seat) == turn.active()).unwrap();
            seen.push((turn.number(), seat));
        }
        if seen.len() > turns && driver.engine().decision().is_some() {
            return Ok(seen.iter().skip(1).map(|(_, seat)| *seat).collect());
        }
        driver.answer_passively()?;
    }
    Err(ScriptError::Stuck("the turns".into()))
}

#[rstest]
#[case::the_next_turn_of_the_other_in_two(2, 0, 1, 1, &[0, 1, 0])]
#[case::two_rules_two_turns(2, 0, 1, 2, &[0, 0, 1])]
#[case::my_own_next_turn_in_two(2, 0, 0, 1, &[1, 1, 0])]
#[case::one_of_three(3, 0, 1, 1, &[2, 0, 1])]
#[case::one_of_four(4, 1, 2, 1, &[3, 0, 1])]
fn each_rule_skips_one_turn_of_the_player_and_ends(
    #[case] seats: usize,
    #[case] active: usize,
    #[case] skipper: usize,
    #[case] rules: usize,
    #[case] expected: &[usize],
) -> Result<(), ScriptError> {
    let scenario = Scenario::new(seats).at(START, active, Step::Main1);
    let setup = scenario.clone().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario)?;
    for _ in 0..rules {
        driver.add_rule(
            Rule::Cant {
                who: vec![Target::Player(pid(skipper))],
                action: Box::new(Prohibited::SkipTurn),
            },
            Duration::UntilTurnSkipped,
        )?;
    }
    let began = play(&mut driver, seats, expected.len())?;
    assert_eq!(began, expected);
    let expired = driver
        .engine()
        .events()
        .iter()
        .filter(|event| matches!(event, Event::RuleExpired { .. }))
        .count();
    assert_eq!(expired, rules);
    assert_eq!(driver.state().rule_effects().len(), 0);
    let replayed = replay(&setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.engine().state_hash());
    Ok(())
}
