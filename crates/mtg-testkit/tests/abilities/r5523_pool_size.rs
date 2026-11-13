//! Goal: "Omnath gets +1/+1 for each unspent green mana you have" counts only the green mana in its controller's pool
//! as it changes: green added grows it, red mana does not, and green kept from step to step keeps counting; at two to
//! four seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_engine::characteristics;
use mtg_engine::turn::Step;
use mtg_testkit::cards::{forest, mountain};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

const R5523_OMNATH: &str = "Omnath, Locus of Mana";

fn r5523_size(driver: &Driver) -> (i32, i32) {
    let id = driver.id_of(R5523_OMNATH).unwrap();
    let found = characteristics(driver.state(), id).unwrap();
    (found.power.unwrap(), found.toughness.unwrap())
}

/// Seat 0 begins its turn with Omnath, two Forests and a Mountain; seat 1 has a Forest.
fn r5523_table(seats: usize) -> Scenario {
    Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .battlefield(
            0,
            [
                ready(card(R5523_OMNATH).unwrap()),
                ready(forest()),
                ready(forest()),
                ready(mountain()),
            ],
        )
        .battlefield(1, [ready(forest())])
}

#[rstest]
fn r5523_omnath_counts_unspent_green(#[values(2, 3, 4)] seats: usize) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(r5523_table(seats))?;
    assert_eq!(r5523_size(&driver), (1, 1), "an empty pool");
    driver.tap_for_mana("Mountain")?;
    assert_eq!(r5523_size(&driver), (1, 1), "red mana does not count");
    driver.tap_for_mana("Forest")?;
    assert_eq!(r5523_size(&driver), (2, 2), "one green");
    driver.tap_for_mana("Forest")?;
    assert_eq!(r5523_size(&driver), (3, 3), "two green");
    driver.pass_until(Step::BeginCombat)?;
    assert_eq!(
        r5523_size(&driver),
        (3, 3),
        "green is kept as the step ends"
    );
    Ok(())
}
