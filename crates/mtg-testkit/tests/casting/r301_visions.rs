//! Goal: "This spell costs {X} less to cast this way, where X is the greatest mana value of a commander you own on the battlefield or in the command zone." (Visions of Glory) takes the mana value of a commander in the command zone off the flashback cost and nothing off the printed cost, at tables of two to four seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use crate::common::casting::lands;
use mtg_core::color::Color;
use mtg_core::house::HouseRules;
use mtg_engine::format::Commander;
use mtg_testkit::cards::{island, plains};
use mtg_testkit::commander_cards::legend;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// Seat 0 in a Commander game with Visions of Glory in the graveyard (or hand), ten Plains and a commander of mana value
/// `value` in the command zone or on the battlefield.
fn table(seats: usize, value: u32, in_play: bool, in_graveyard: bool) -> Scenario {
    let commander = legend("Cmd", value.saturating_sub(1), &[Color::White], 2, 2);
    let mut permanents = lands(10, plains);
    let mut scenario = Scenario::new(seats)
        .format(Arc::new(Commander))
        .house_rules(HouseRules {
            starting_life: Some(40),
            ..HouseRules::default()
        });
    if in_play {
        permanents.push(ready(commander));
    } else {
        scenario = scenario.commanders(0, [commander]);
    }
    scenario = scenario.battlefield(0, permanents);
    scenario = if in_graveyard {
        scenario.graveyard_named(0, ["Visions of Glory"])
    } else {
        scenario.hand_named(0, ["Visions of Glory"])
    };
    for seat in 0..seats {
        scenario = scenario.library(seat, (0..10).map(|_| island()));
    }
    scenario
}

/// The mana value of the cost asked to cast Visions of Glory.
fn asked(driver: &mut Driver) -> Result<u32, ScriptError> {
    driver.begin_cast("Visions of Glory")?;
    Ok(driver.pay_options()?.cost.mana_value())
}

#[rstest]
#[case::two_seats_command_zone(2, 5, false)]
#[case::three_seats_command_zone(3, 4, false)]
#[case::four_seats_command_zone(4, 7, false)]
fn the_commander_takes_its_mana_value_off_the_flashback_cost(
    #[case] seats: usize,
    #[case] value: u32,
    #[case] in_play: bool,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table(seats, value, in_play, true))?;
    assert_eq!(asked(&mut driver)?, 10 - value);
    Ok(())
}

#[test]
fn the_printed_cost_is_not_reduced() -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table(2, 5, false, false))?;
    assert_eq!(asked(&mut driver)?, 5);
    Ok(())
}
