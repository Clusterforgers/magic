//! Goal: "Whenever players finish voting" triggers once per vote at two to four seats and reads who voted with whom:
//! Grudge Keeper drains each opponent who voted for a choice its controller didn't vote for, Erestor of the Council
//! gives a Treasure to each opponent who voted the same and scries for the others, Model of Unity asks its controller and
//! the like-minded opponents whether they scry, a near miss (everyone voting alike, everyone voting apart) changes the
//! count, and the log replays.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_engine::choose::{ChoiceOption, OptionChoice, YesNoChoice};
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::replay;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{bear, plains};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::pid;

/// Seat 0 holds Lieutenants of the Guard with five Plains and controls `watcher`; every seat has a library.
fn r379_table(seats: usize, watcher: &str) -> Scenario {
    let mut ours = vec![card(watcher).unwrap()];
    ours.extend((0..5).map(|_| plains()));
    let mut scenario = Scenario::new(seats)
        .at(2, 0, Step::Main1)
        .hand(0, [card("Lieutenants of the Guard").unwrap()])
        .battlefield(0, ours.into_iter().map(ready).collect::<Vec<_>>());
    for seat in 0..seats {
        scenario = scenario.library(seat, (0..6).map(|_| bear()));
    }
    scenario
}

/// Casts Lieutenants of the Guard and answers until the stack is empty: each seat votes `strength` when its entry is
/// true, `numbers` otherwise; the seats asked a yes-no question are returned in order, each answered yes.
fn r379_vote(driver: &mut Driver, strength: &[bool]) -> Result<Vec<usize>, ScriptError> {
    driver.cast("Lieutenants of the Guard")?;
    let mut asked = Vec::new();
    for _ in 0..120 {
        let decision = driver.decision()?.clone();
        match &decision.kind {
            DecisionKind::Priority(_)
                if driver.state().stack().is_empty()
                    && driver.state().pending_triggers().is_empty() =>
            {
                return Ok(asked);
            }
            DecisionKind::ChooseOption(options)
                if options
                    .options
                    .iter()
                    .any(|o| *o == ChoiceOption::Label("strength".to_owned())) =>
            {
                let wanted = if strength[decision.player.index()] {
                    "strength"
                } else {
                    "numbers"
                };
                let at = options
                    .options
                    .iter()
                    .position(|o| *o == ChoiceOption::Label(wanted.to_owned()))
                    .unwrap();
                driver.answer(Answer::ChooseOption(OptionChoice(at)))?;
            }
            DecisionKind::YesNo(_) => {
                asked.push(decision.player.index());
                driver.answer(Answer::YesNo(YesNoChoice(true)))?;
            }
            DecisionKind::PayMana(_) => {
                driver.answer(Answer::PayMana(PayChoice::Auto))?;
            }
            _ => {
                driver.answer(Driver::passive_answer(&decision))?;
            }
        }
    }
    Err(ScriptError::Stuck("the stack did not empty".to_owned()))
}

fn r379_replays(driver: &Driver, scenario: Scenario) {
    let setup = scenario.setup().unwrap();
    let replayed = replay(&setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.engine().state_hash());
}

/// The votes of the seats: seat 0 votes strength, the others as `pattern` says (cycled).
fn r379_ballots(seats: usize, pattern: &[bool]) -> Vec<bool> {
    (0..seats)
        .map(|seat| seat == 0 || pattern[(seat - 1) % pattern.len()])
        .collect()
}

#[rstest]
#[case::mixed(&[true, false])]
#[case::near_miss_all_alike(&[true])]
#[case::near_miss_all_apart(&[false])]
fn r379_grudge_keeper_drains_who_voted_apart(
    #[values(2, 3, 4)] seats: usize,
    #[case] pattern: &[bool],
) -> Result<(), ScriptError> {
    let ballots = r379_ballots(seats, pattern);
    let mut driver = Driver::from_scenario(r379_table(seats, "Grudge Keeper"))?;
    r379_vote(&mut driver, &ballots)?;
    for (seat, alike) in ballots.iter().enumerate().skip(1) {
        let life = driver.state().player(pid(seat)).unwrap().life();
        assert_eq!(life, if *alike { 20 } else { 18 }, "seat {seat}");
    }
    assert_eq!(driver.state().player(pid(0)).unwrap().life(), 20);
    r379_replays(&driver, r379_table(seats, "Grudge Keeper"));
    Ok(())
}

#[rstest]
#[case::mixed(&[true, false])]
#[case::near_miss_all_alike(&[true])]
#[case::near_miss_all_apart(&[false])]
fn r379_erestor_rewards_who_voted_alike(
    #[values(2, 3, 4)] seats: usize,
    #[case] pattern: &[bool],
) -> Result<(), ScriptError> {
    let ballots = r379_ballots(seats, pattern);
    let mut driver = Driver::from_scenario(r379_table(seats, "Erestor of the Council"))?;
    let hand = driver.state().zone(Zone::Hand(pid(0))).len();
    r379_vote(&mut driver, &ballots)?;
    let state = driver.state();
    for (seat, alike) in ballots.iter().enumerate() {
        let treasures = state
            .zone(Zone::Battlefield)
            .iter()
            .filter(|id| {
                let object = state.object(**id).unwrap();
                object.controller() == pid(seat) && object.card().name == "Treasure"
            })
            .count();
        let expected = usize::from(seat != 0 && *alike);
        assert_eq!(treasures, expected, "seat {seat}");
    }
    // The Lieutenants left the hand; Erestor's controller drew one card.
    assert_eq!(state.zone(Zone::Hand(pid(0))).len(), hand);
    r379_replays(&driver, r379_table(seats, "Erestor of the Council"));
    Ok(())
}

#[rstest]
#[case::mixed(&[true, false])]
#[case::near_miss_all_alike(&[true])]
#[case::near_miss_all_apart(&[false])]
fn r379_model_of_unity_asks_the_like_minded(
    #[values(2, 3, 4)] seats: usize,
    #[case] pattern: &[bool],
) -> Result<(), ScriptError> {
    let ballots = r379_ballots(seats, pattern);
    let mut driver = Driver::from_scenario(r379_table(seats, "Model of Unity"))?;
    let asked = r379_vote(&mut driver, &ballots)?;
    let expected: Vec<usize> = (0..seats).filter(|seat| ballots[*seat]).collect();
    assert_eq!(asked, expected, "you, then each opponent who voted alike");
    r379_replays(&driver, r379_table(seats, "Model of Unity"));
    Ok(())
}
