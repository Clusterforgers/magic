//! Goal: an annihilator of a counted amount (Ulamog, the Defiler: "annihilator X, where X is the number of +1/+1 counters on it", CR 702.86) makes the defending player sacrifice as many permanents as the counters on it as the trigger resolves; at tables of two to four seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_cards::value::CounterKind;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{bear, giant, island};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready, sick};
use rstest::rstest;

fn settle(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..60 {
        if driver.state().stack().is_empty()
            && driver.state().resolving().is_none()
            && driver.state().pending_triggers().is_empty()
        {
            return Ok(());
        }
        driver.answer_passively()?;
    }
    Err(ScriptError::Stuck("the stack".into()))
}

#[rstest]
#[case::two_seats_two(2, 2)]
#[case::three_seats_three(3, 3)]
#[case::four_seats_one(4, 1)]
fn annihilator_counts_the_counters(
    #[case] seats: usize,
    #[case] counters: u32,
) -> Result<(), ScriptError> {
    let ulamog = mtg_testkit::with_counters(
        ready(card("Ulamog, the Defiler").unwrap()),
        CounterKind::PlusOnePlusOne,
        counters,
    );
    let mut scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .battlefield(0, [ulamog])
        .battlefield(1, [sick(bear()), sick(bear()), sick(bear()), sick(giant())]);
    for seat in 0..seats {
        scenario = scenario.library(seat, (0..10).map(|_| island()));
    }
    let mut driver = Driver::from_scenario(scenario)?;
    driver.pass_until(Step::DeclareAttackers)?;
    driver.attack(&[("Ulamog, the Defiler", 1)])?;
    settle(&mut driver)?;
    let left = driver.state().zone(Zone::Battlefield).len() - 1;
    assert_eq!(left, 4 - usize::try_from(counters).unwrap());
    Ok(())
}
