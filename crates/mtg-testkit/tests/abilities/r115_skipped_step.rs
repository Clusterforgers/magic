//! Goal: the beginning of a step is replaced (CR 614.10b) at 2 to 4 seats: under Fasting its controller is asked as
//! their draw step would begin; a yes skips the step (no card drawn, the precombat main phase begins) and gains 2 life, a
//! no draws as usual (and Fasting's draw trigger destroys it); another player's draw step is not asked about; each log
//! replays to the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_engine::choose::YesNoChoice;
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::forest;
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::copying::replays;
use crate::common::{names, pid};

/// Seat `owner` controls Fasting; every library holds Forests; the last seat's turn ends.
fn table(seats: usize, owner: usize) -> Scenario {
    let mut scenario = Scenario::new(seats).at(1, seats - 1, Step::End);
    for seat in 0..seats {
        scenario = scenario.library(seat, (0..5).map(|_| forest()).collect::<Vec<_>>());
    }
    scenario.battlefield(owner, [ready(card("Fasting").unwrap())])
}

/// Plays on to seat 0's precombat main phase, answering the yes or no questions with `skip`; how many were asked.
fn to_main(driver: &mut Driver, skip: bool) -> Result<usize, ScriptError> {
    let mut asked = 0;
    for _ in 0..200 {
        let decision = driver.decision()?.clone();
        if driver.state().turn().number() == 2 && driver.state().turn().step() == Step::Main1 {
            return Ok(asked);
        }
        if let DecisionKind::YesNo(_) = decision.kind {
            asked += 1;
            driver.answer(Answer::YesNo(YesNoChoice(skip)))?;
        } else {
            driver.answer_passively()?;
        }
    }
    Err(ScriptError::Stuck("the main phase".into()))
}

// (owner of Fasting, skip, questions, hand after, life after, Fasting left)
#[rstest]
#[case::skipped(0, true, 1, 0, 22, true)]
#[case::drawn(0, false, 1, 1, 20, false)]
#[case::another_players(1, true, 0, 1, 20, true)]
fn the_draw_step_may_be_skipped_for_life(
    #[values(2, 3, 4)] seats: usize,
    #[case] owner: usize,
    #[case] skip: bool,
    #[case] questions: usize,
    #[case] hand: usize,
    #[case] life: i64,
    #[case] kept: bool,
) -> Result<(), ScriptError> {
    let setup = table(seats, owner).setup().unwrap();
    let mut driver = Driver::from_scenario(table(seats, owner))?;
    assert_eq!(to_main(&mut driver, skip)?, questions);
    assert_eq!(driver.state().zone(Zone::Hand(pid(0))).len(), hand);
    assert_eq!(
        i64::from(driver.state().player(pid(0)).unwrap().life()),
        life
    );
    let battlefield = names(driver.state(), Zone::Battlefield);
    assert_eq!(
        battlefield.contains(&"Fasting".to_owned()),
        kept,
        "{battlefield:?}"
    );
    replays(&driver, &setup);
    Ok(())
}
