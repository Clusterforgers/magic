//! Goal: a count of different values of a characteristic ("for each different power among creatures you control") plays
//! as printed at tables of two, three and four seats: Golden Ratio draws one card for each different power among its
//! caster's creatures, never counting another player's creatures or a repeated power, and the log replays to the same
//! state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_core::ids::PlayerId;
use mtg_engine::decision::{Answer, PayChoice};
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{bear, forest, free_creature, island};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::copying::replays;

#[rstest]
#[case::one_power(&[2, 2, 2], 1)]
#[case::three_powers(&[1, 2, 3], 3)]
#[case::zero_counts(&[0, 2, 2, 5], 3)]
fn w2c_golden_ratio_draws_for_each_different_power(
    #[values(2, 3, 4)] seats: usize,
    #[case] powers: &[i32],
    #[case] draws: usize,
) -> Result<(), ScriptError> {
    let mut mine: Vec<_> = powers
        .iter()
        .map(|power| ready(free_creature(&format!("Power {power}"), *power, 4)))
        .collect();
    mine.extend((0..2).map(|_| ready(forest())));
    mine.extend((0..2).map(|_| ready(island())));
    let mut scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .hand(0, [card("Golden Ratio").unwrap()])
        .library(0, (0..10).map(|_| bear()))
        .battlefield(0, mine);
    for seat in 1..seats {
        scenario = scenario.battlefield(seat, [ready(free_creature("Other", 7, 7))]);
    }
    let setup = scenario.clone().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario)?;
    driver.begin_cast("Golden Ratio")?;
    driver.answer(Answer::PayMana(PayChoice::Auto))?;
    for _ in 0..50 {
        if driver.state().stack().is_empty() && driver.state().resolving().is_none() {
            break;
        }
        driver.answer_passively()?;
    }
    assert_eq!(driver.state().zone(Zone::Hand(PlayerId(0))).len(), draws);
    replays(&driver, &setup);
    Ok(())
}
