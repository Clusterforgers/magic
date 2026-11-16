//! Goal: a loyalty ability with "Activate only if ..." (CR 602.5b, 606.3; Jace, Reality Sculptor's 0: "Activate only if there are twenty-five or more loyalty counters among Jaces you control") is offered only while the condition holds, counted over every Jace its controller controls, and exiles all but the bottom card of each opponent's library; at tables of two to four seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use crate::common::pid;
use mtg_cards::CardDefinition;
use mtg_cards::value::CounterKind;
use mtg_core::card::CardType;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::island;
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready, with_counters};
use rstest::rstest;

/// Jace's third printed ability: the 0.
const ZERO: usize = 2;

/// A nonlegendary Jace planeswalker with no abilities.
fn doll() -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new("Jace Doll")
            .with_type(CardType::Planeswalker)
            .with_subtype("Jace")
            .with_loyalty(1),
    )
}

/// Seat 0 controls Jace, Reality Sculptor with `jace` loyalty counters and a Jace Doll with `doll_loyalty`; every seat
/// has ten cards in its library.
fn with_loyalty(seats: usize, jace: u32, doll_loyalty: u32) -> Scenario {
    let real = card("Jace, Reality Sculptor").unwrap();
    let mut scenario = Scenario::new(seats).battlefield(
        0,
        [
            with_counters(ready(real), CounterKind::Loyalty, jace),
            with_counters(ready(doll()), CounterKind::Loyalty, doll_loyalty),
        ],
    );
    for seat in 0..seats {
        scenario = scenario.library(seat, (0..10).map(|_| island()));
    }
    scenario
}

fn loyalty_among_jaces(driver: &Driver) -> u32 {
    driver
        .state()
        .zone(Zone::Battlefield)
        .iter()
        .filter_map(|id| driver.state().object(*id))
        .filter_map(|object| object.permanent())
        .map(|permanent| permanent.counters_of(&CounterKind::Loyalty))
        .sum()
}

#[rstest]
#[case::two_seats_twenty_five(2, 20, 5, true)]
#[case::three_seats_twenty_four(3, 20, 4, false)]
#[case::four_seats_thirty(4, 25, 5, true)]
#[case::two_seats_one_short(2, 23, 1, false)]
fn the_zero_is_offered_only_with_twenty_five_loyalty(
    #[case] seats: usize,
    #[case] jace: u32,
    #[case] doll: u32,
    #[case] offered: bool,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(with_loyalty(seats, jace, doll))?;
    let total = loyalty_among_jaces(&driver);
    assert_eq!(total >= 25, offered, "{total} loyalty among the Jaces");
    let abilities = driver.offered_abilities("Jace, Reality Sculptor")?;
    assert_eq!(abilities.contains(&ZERO), offered);
    if !offered {
        return Ok(());
    }
    driver.begin_ability("Jace, Reality Sculptor", ZERO)?;
    for _ in 0..40 {
        if driver.state().stack().is_empty() && driver.state().resolving().is_none() {
            break;
        }
        driver.answer_passively()?;
    }
    assert_eq!(driver.state().zone(Zone::Library(pid(0))).len(), 10);
    for seat in 1..seats {
        assert_eq!(
            driver.state().zone(Zone::Library(pid(seat))).len(),
            1,
            "all but the bottom card of seat {seat}"
        );
    }
    assert_eq!(driver.state().zone(Zone::Exile).len(), 9 * (seats - 1));
    Ok(())
}
