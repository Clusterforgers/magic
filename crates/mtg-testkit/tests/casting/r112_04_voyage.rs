//! Goal: Last Voyage of the _____ becomes an Aura with enchant creature as it enters, returns a creature card from its
//! controller's graveyard and enchants it, at two to four seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_core::card::Subtype;
use mtg_engine::turn::Step;
use mtg_testkit::cards::{bear, swamp};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario};
use rstest::rstest;

use crate::common::casting::lands;

fn settle(driver: &mut Driver) {
    for _ in 0..200 {
        if driver.state().stack().is_empty() && driver.state().resolving().is_none() {
            return;
        }
        driver.answer_passively().unwrap();
    }
    panic!("the stack never emptied");
}

#[rstest]
fn r112_the_voyage_returns_a_creature_and_enchants_it(#[values(2, 3, 4)] seats: usize) {
    let scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .hand(0, [card("Last Voyage of the _____").unwrap()])
        .graveyard(0, [bear()])
        .battlefield(0, lands(4, swamp));
    let mut driver = Driver::from_scenario(scenario).unwrap();
    driver
        .cast_tapping(
            "Last Voyage of the _____",
            &["Swamp", "Swamp", "Swamp", "Swamp"],
        )
        .unwrap();
    settle(&mut driver);
    let bear = driver.id_of("Grizzly Bears").unwrap();
    let voyage = driver.id_of("Last Voyage of the _____").unwrap();
    assert_eq!(driver.state().host_of(voyage), Some(bear));
    let chars = mtg_engine::characteristics::characteristics(driver.state(), voyage).unwrap();
    assert!(chars.has_subtype(&[], &Subtype("Aura".to_owned())));
}
