//! Goal: "This spell costs {X} less to cast this way, where X is the greatest mana value of a commander you own on the battlefield or in the command zone" takes that much generic mana off the flashback cost alone, with the commander in the command zone or on the battlefield, and takes nothing without a commander; at 2 to 4 seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use crate::common::casting::{lands, pay};
use crate::common::on_battlefield;
use mtg_core::color::Color;
use mtg_engine::format::Commander;
use mtg_testkit::cards::plains;
use mtg_testkit::commander_cards::legend;
use mtg_testkit::{Driver, Scenario, ScriptError};
use rstest::rstest;

/// Seat 0 has Visions of Glory in its graveyard and `count` Plains; with `commander`, the commander "Cmd" of mana value
/// 5 ({4}{W}) waits in its command zone.
fn table(seats: usize, count: usize, commander: bool) -> Scenario {
    let scenario = Scenario::new(seats)
        .graveyard_named(0, ["Visions of Glory"])
        .battlefield(0, lands(count, plains));
    if commander {
        scenario
            .format(Arc::new(Commander))
            .commanders(0, [legend("Cmd", 4, &[Color::White], 2, 2)])
    } else {
        scenario
    }
}

#[rstest]
fn the_commander_in_the_command_zone_takes_its_mana_value_off_the_flashback_cost(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table(seats, 5, true))?;
    driver.begin_cast("Visions of Glory")?;
    assert_eq!(driver.pay_options()?.cost.to_string(), "{3}{W}{W}");
    Ok(())
}

#[rstest]
fn the_commander_on_the_battlefield_still_counts(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table(seats, 10, true))?;
    driver.cast("Cmd")?;
    for _ in 0..seats {
        driver.pass()?;
    }
    assert!(on_battlefield(driver.state(), "Cmd").is_some());
    driver.begin_cast("Visions of Glory")?;
    assert_eq!(driver.pay_options()?.cost.to_string(), "{3}{W}{W}");
    pay(&mut driver)?;
    Ok(())
}

#[rstest]
#[case::without_a_commander_the_whole_cost(10, false, true)]
#[case::nine_lands_do_not_pay_it(9, false, false)]
#[case::four_lands_do_not_pay_the_reduced_cost(4, true, false)]
fn the_flashback_is_offered_only_when_the_lands_pay_what_it_costs(
    #[values(2, 3, 4)] seats: usize,
    #[case] count: usize,
    #[case] commander: bool,
    #[case] offered: bool,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table(seats, count, commander))?;
    assert_eq!(driver.begin_cast("Visions of Glory").is_ok(), offered);
    if offered {
        assert_eq!(driver.pay_options()?.cost.to_string(), "{8}{W}{W}");
    }
    Ok(())
}
