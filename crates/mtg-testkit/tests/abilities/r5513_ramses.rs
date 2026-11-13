//! Goal: Ramses, Assassin Lord wins the game for its controller when a player loses the game for any reason, if an
//! Assassin they controlled attacked that player this turn (a concession counts, per its rulings), at three and four
//! seats; a player attacked only by a non-Assassin, or not attacked at all, loses without the win.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common::pid;
use mtg_engine::turn::Step;
use mtg_engine::{Event, GameResult};
use mtg_testkit::cards::free_creature;
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

const RAMSES: &str = "Ramses, Assassin Lord";

/// Answers passively until the stack is empty, or the game ended.
fn r5513_settle(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..100 {
        if r5513_ended(driver) || driver.state().stack().is_empty() {
            return Ok(());
        }
        driver.answer_passively()?;
    }
    Err(ScriptError::Stuck("the stack".into()))
}

/// Whether the game ended.
fn r5513_ended(driver: &Driver) -> bool {
    driver
        .engine()
        .events()
        .iter()
        .any(|event| matches!(event, Event::GameEnded { .. }))
}

#[rstest]
#[case::assassin_attacked_the_loser(3, RAMSES, 1, 1, true)]
#[case::four_seats(4, RAMSES, 2, 2, true)]
#[case::the_loser_was_not_attacked(3, RAMSES, 1, 2, false)]
#[case::a_non_assassin_attacked(4, "Pip", 1, 1, false)]
fn r5513_ramses_wins_when_a_player_its_assassins_attacked_loses(
    #[case] seats: usize,
    #[case] attacker: &str,
    #[case] attacked: usize,
    #[case] loser: usize,
    #[case] wins: bool,
) -> Result<(), ScriptError> {
    let mut scenario = Scenario::new(seats).battlefield(
        0,
        [
            ready(card(RAMSES).unwrap()),
            ready(free_creature("Pip", 1, 1)),
        ],
    );
    for seat in 0..seats {
        scenario = scenario.library(seat, (0..8).map(|_| mtg_testkit::cards::bear()));
    }
    let mut driver = Driver::from_scenario(scenario.at(1, 0, Step::Main1))?;
    driver.pass_until(Step::DeclareAttackers)?;
    driver.attack(&[(attacker, attacked)])?;
    driver.pass_until(Step::Main2)?;
    assert!(!r5513_ended(&driver));
    driver.concede(pid(loser))?;
    r5513_settle(&mut driver)?;
    let won = driver.engine().events().iter().any(|event| {
        matches!(
            event,
            Event::GameEnded {
                result: GameResult::Winner(winner)
            } if *winner == pid(0)
        )
    });
    assert_eq!(won, wins);
    Ok(())
}
