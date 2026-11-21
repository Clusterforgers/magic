//! Goal: a permanent's untap in its controller's untap step is replaced (CR 502.3, 614.1) at 2 to 4 seats: Bewitching
//! Leechcraft's enchanted creature removes a +1/+1 counter instead and untaps only if it did, so without a counter it
//! stays tapped; an untap in another step is not replaced; each log replays to the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_cards::value::CounterKind;
use mtg_engine::turn::Step;
use mtg_testkit::cards::free_creature;
use mtg_testkit::corpus::card;
use mtg_testkit::scenario::{attached_to, tapped, with_counters};
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::copying::replays;

/// Seat 0's tapped Bear with `counters` +1/+1 counters, enchanted by Bewitching Leechcraft; the last seat's turn ends.
fn table(seats: usize, counters: u32) -> Scenario {
    let bear = tapped(free_creature("Bear", 2, 2));
    let bear = if counters == 0 {
        bear
    } else {
        with_counters(bear, CounterKind::PlusOnePlusOne, counters)
    };
    Scenario::new(seats)
        .at(1, seats - 1, Step::End)
        .battlefield(
            0,
            [
                bear,
                attached_to(ready(card("Bewitching Leechcraft").unwrap()), 0, 0),
            ],
        )
}

// (counters before, untapped after, counters after)
#[rstest]
#[case::two_counters(2, true, 1)]
#[case::one_counter(1, true, 0)]
#[case::no_counter(0, false, 0)]
fn the_untap_step_takes_a_counter_or_keeps_it_tapped(
    #[values(2, 3, 4)] seats: usize,
    #[case] before: u32,
    #[case] untapped: bool,
    #[case] after: u32,
) -> Result<(), ScriptError> {
    let setup = table(seats, before).setup().unwrap();
    let mut driver = Driver::from_scenario(table(seats, before))?;
    driver.idle_until(2, Step::Upkeep)?;
    let bear = driver.id_of("Bear").unwrap();
    let permanent = driver.state().object(bear).unwrap().permanent().unwrap();
    assert_eq!(!permanent.tapped(), untapped);
    assert_eq!(permanent.counters_of(&CounterKind::PlusOnePlusOne), after);
    replays(&driver, &setup);
    Ok(())
}
