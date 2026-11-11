//! Goal: chroma counts the mana symbols of one color in mana costs at two to four seats: a symbol of another color and
//! generic mana count nothing, and the count follows the objects it reads (the cards in its controller's graveyard, each
//! creature's own cost, the creature sacrificed to pay).
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use crate::common::casting::{lands, pay};
use crate::common::{names, pid};
use mtg_cards::CardDefinition;
use mtg_core::color::Color;
use mtg_engine::characteristics::characteristics;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{creature, mountain};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// The power and toughness of the permanent named `name`.
fn stats(driver: &Driver, name: &str) -> (Option<i32>, Option<i32>) {
    let id = driver.id_of(name).unwrap();
    let chars = characteristics(driver.state(), id).unwrap();
    (chars.power, chars.toughness)
}

/// A creature card with a mana cost of `generic` and the colored symbols.
fn costed(name: &str, generic: u32, colored: &[Color]) -> Arc<CardDefinition> {
    creature(name, generic, colored, 1, 1)
}

#[rstest]
#[case::two_seats(2)]
#[case::three_seats(3)]
#[case::four_seats(4)]
fn umbra_stalker_counts_the_black_symbols_of_its_controllers_graveyard(
    #[case] seats: usize,
) -> Result<(), ScriptError> {
    let mut scenario = Scenario::new(seats)
        .battlefield(0, vec![ready(card("Umbra Stalker").unwrap())])
        .graveyard(
            0,
            vec![
                costed("Two", 1, &[Color::Black, Color::Black]),
                costed("Red", 0, &[Color::Red]),
                costed("One", 3, &[Color::Black]),
            ],
        );
    // An opponent's graveyard is not counted.
    scenario = scenario.graveyard(1, vec![costed("Theirs", 0, &[Color::Black; 3])]);
    let driver = Driver::from_scenario(scenario)?;
    assert_eq!(stats(&driver, "Umbra Stalker"), (Some(3), Some(3)));
    Ok(())
}

#[rstest]
#[case::two_seats(2)]
#[case::four_seats(4)]
fn light_from_within_gives_each_creature_its_own_white_symbols(
    #[case] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = Scenario::new(seats)
        .battlefield(
            0,
            vec![
                ready(card("Light from Within").unwrap()),
                ready(costed("Saint", 1, &[Color::White, Color::White])),
                ready(costed("Brute", 2, &[Color::Red])),
            ],
        )
        .battlefield(1, vec![ready(costed("Foe", 0, &[Color::White]))]);
    let driver = Driver::from_scenario(scenario)?;
    assert_eq!(stats(&driver, "Saint"), (Some(3), Some(3)));
    assert_eq!(stats(&driver, "Brute"), (Some(1), Some(1)));
    assert_eq!(
        stats(&driver, "Foe"),
        (Some(1), Some(1)),
        "only creatures you control"
    );
    Ok(())
}

#[test]
fn fiery_bombardment_deals_the_red_symbols_of_the_sacrificed_creature() -> Result<(), ScriptError> {
    let scenario = Scenario::new(2).battlefield(0, {
        let mut mine = lands(2, mountain);
        mine.push(ready(card("Fiery Bombardment").unwrap()));
        mine.push(ready(costed("Hothead", 1, &[Color::Red, Color::Red])));
        mine
    });
    let mut driver = Driver::from_scenario(scenario)?;
    let life = driver.state().player(pid(1)).unwrap().life();
    driver.begin_activation("Fiery Bombardment")?;
    let foe = driver.target_player(1);
    driver.choose_targets(&[foe])?;
    pay(&mut driver)?;
    while !driver.state().zone(Zone::Stack).is_empty() {
        driver.pass()?;
    }
    assert_eq!(driver.state().player(pid(1)).unwrap().life(), life - 2);
    assert_eq!(names(driver.state(), Zone::Graveyard(pid(0))), ["Hothead"]);
    Ok(())
}
