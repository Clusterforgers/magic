//! Goal: God-Eternal Kefnet at 2 to 4 seats: the first card drawn may be revealed as it is drawn; a revealed sorcery is
//! copied from the hand (CR 707.12) and the copy may be cast for {2} less (a rule fixed to that copy as it begins,
//! CR 611.2c), so two Islands pay for a {3}{U} copy; an unrevealed card makes no copy; each log replays to the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_engine::choose::{CardsChoice, YesNoChoice};
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{forest, island};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::copying::replays;
use crate::common::pid;

/// Seat 0 controls Kefnet and two Islands in its upkeep, with Coastal Discovery on top of its library.
fn table(seats: usize) -> Scenario {
    let mut scenario = Scenario::new(seats)
        .at(2, 0, Step::Upkeep)
        .battlefield(
            0,
            [
                ready(card("God-Eternal Kefnet").unwrap()),
                ready(island()),
                ready(island()),
            ],
        )
        .library(
            0,
            [
                card("Coastal Discovery").unwrap(),
                forest(),
                forest(),
                forest(),
            ],
        );
    for seat in 1..seats {
        scenario = scenario.library(seat, [forest(), forest()]);
    }
    scenario
}

#[rstest]
#[case::revealed_and_copied(true, 3)]
#[case::kept_hidden(false, 1)]
fn kefnet_copies_a_revealed_sorcery_for_two_less(
    #[values(2, 3, 4)] seats: usize,
    #[case] reveal: bool,
    #[case] drawn: usize,
) -> Result<(), ScriptError> {
    let setup = table(seats).setup().unwrap();
    let mut driver = Driver::from_scenario(table(seats))?;
    let hand = driver.state().zone(Zone::Hand(pid(0))).len();
    let mut asked = 0;
    for _ in 0..200 {
        let decision = driver.decision()?.clone();
        match &decision.kind {
            DecisionKind::Priority(_)
                if driver.state().turn().step() == Step::Main1
                    && driver.state().stack().is_empty() =>
            {
                break;
            }
            // The reveal is a pick of the card drawn, or of none.
            DecisionKind::ChooseCards(options) => {
                let picked = if reveal {
                    options.from.clone()
                } else {
                    Vec::new()
                };
                driver.answer(Answer::ChooseCards(CardsChoice(picked)))?;
            }
            DecisionKind::YesNo(_) => {
                asked += 1;
                driver.answer(Answer::YesNo(YesNoChoice(true)))?;
            }
            DecisionKind::PayMana(_) => {
                driver.answer(Answer::PayMana(PayChoice::Auto))?;
            }
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    assert_eq!(asked, usize::from(reveal), "the copy is offered once");
    assert_eq!(driver.state().zone(Zone::Hand(pid(0))).len(), hand + drawn);
    replays(&driver, &setup);
    Ok(())
}
