//! Goal: Reverse the Sands plays as printed at two to four seats: its caster gives each player one of the life totals the
//! players had, each total once and never split (the Reverse the Sands rulings), so totals can be swapped between any
//! players or left as they are, and each player gains or loses the difference.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_engine::choose::{ChoiceOption, OptionChoice};
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::replay;
use mtg_engine::turn::Step;
use mtg_testkit::cards::plains;
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::pid;

/// Seat `i` starts at `5 + 5 * i` life; seat 0 holds Reverse the Sands and eight Plains.
fn r6501_table(seats: usize) -> Scenario {
    let mut scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .battlefield(0, (0..8).map(|_| ready(plains())))
        .hand(0, [card("Reverse the Sands").unwrap()]);
    for seat in 0..seats {
        scenario = scenario.life(seat, 5 + 5 * i32::try_from(seat).unwrap());
    }
    scenario
}

/// Casts Reverse the Sands and gives each seat, in turn order, the total `wanted` names for it; returns how many
/// questions were asked.
fn r6501_redistribute(driver: &mut Driver, wanted: &[i32]) -> Result<usize, ScriptError> {
    driver.cast_tapping("Reverse the Sands", &["Plains"; 8])?;
    let mut asked = 0;
    for _ in 0..60 {
        if driver.state().stack().is_empty() && driver.state().resolving().is_none() {
            return Ok(asked);
        }
        if let DecisionKind::ChooseOption(options) = &driver.decision()?.kind {
            // Turn order from seat 0: the question is for the seat numbered as the questions so far.
            let label = format!("{} life", wanted[asked]);
            let at = options
                .options
                .iter()
                .position(|option| matches!(option, ChoiceOption::Label(l) if *l == label))
                .expect("the total is offered");
            driver.answer(Answer::ChooseOption(OptionChoice(at)))?;
            asked += 1;
        } else {
            driver.answer_passively()?;
        }
    }
    Err(ScriptError::Stuck("the stack".into()))
}

fn r6501_lives(driver: &Driver, seats: usize) -> Vec<i32> {
    (0..seats)
        .map(|seat| driver.state().player(pid(seat)).unwrap().life())
        .collect()
}

#[rstest]
#[case::the_caster_and_the_last_seat_swap(true)]
#[case::every_total_stays(false)]
fn r6501_reverse_the_sands_gives_each_player_one_total(
    #[values(2, 3, 4)] seats: usize,
    #[case] swap: bool,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(r6501_table(seats))?;
    let before = r6501_lives(&driver, seats);
    let mut wanted = before.clone();
    if swap {
        wanted.swap(0, seats - 1);
    }
    let asked = r6501_redistribute(&mut driver, &wanted)?;
    // The last player gets the one total left without a question.
    assert_eq!(asked, seats - 1);
    assert_eq!(r6501_lives(&driver, seats), wanted);
    let mut sorted = r6501_lives(&driver, seats);
    sorted.sort_unstable();
    assert_eq!(sorted, before, "no total is split or made up");
    let state = replay(driver.engine().setup(), driver.engine().events()).unwrap();
    assert_eq!(state.state_hash(), driver.state().state_hash());
    Ok(())
}

#[rstest]
fn r6501_equal_totals_ask_nothing(#[values(2, 3, 4)] seats: usize) -> Result<(), ScriptError> {
    let mut scenario = r6501_table(seats);
    for seat in 0..seats {
        scenario = scenario.life(seat, 12);
    }
    let mut driver = Driver::from_scenario(scenario)?;
    let asked = r6501_redistribute(&mut driver, &vec![12; seats])?;
    assert_eq!(asked, 0);
    assert_eq!(r6501_lives(&driver, seats), vec![12; seats]);
    Ok(())
}
