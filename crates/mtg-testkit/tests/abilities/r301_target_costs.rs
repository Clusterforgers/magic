//! Goal: "This ability costs {1} less to activate if it targets a creature with power 3 or less." (Raft Security Officer) takes {1} off when the target has power 3 or less and nothing for a bigger one, at tables of two to four seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common::pid;
use mtg_engine::turn::Step;
use mtg_testkit::cards::{free_creature, island, plains};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// Seat 0 with Raft Security Officer and two Plains; the last seat with a 2/2 and a 5/5.
fn table(seats: usize) -> Scenario {
    let mut scenario = Scenario::new(seats)
        .at(u32::try_from(seats).unwrap() + 1, 0, Step::Main1)
        .battlefield(
            0,
            [
                ready(card("Raft Security Officer").unwrap()),
                ready(plains()),
                ready(plains()),
            ],
        )
        .battlefield(
            seats - 1,
            [
                ready(free_creature("Small", 2, 2)),
                ready(free_creature("Big", 5, 5)),
            ],
        );
    for seat in 0..seats {
        scenario = scenario.library(seat, (0..10).map(|_| island()));
    }
    scenario
}

#[rstest]
#[case::two_seats_small(2, "Small", 1)]
#[case::three_seats_big(3, "Big", 2)]
#[case::four_seats_small(4, "Small", 1)]
fn the_officer_costs_less_against_a_small_creature(
    #[case] seats: usize,
    #[case] target: &str,
    #[case] mana: u32,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table(seats))?;
    driver.begin_activation("Raft Security Officer")?;
    let at = driver.target_named(target)?;
    driver.choose_targets(&[at])?;
    assert_eq!(driver.pay_options()?.cost.mana_value(), mana);
    assert_eq!(driver.decision()?.player, pid(0));
    Ok(())
}
