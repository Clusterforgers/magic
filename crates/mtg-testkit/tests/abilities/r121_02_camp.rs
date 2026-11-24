//! Goal: C.A.M.P. triggers when the fortified land is tapped for mana, at two to four seats: the target creature gets a
//! +1/+1 counter, a Junk token comes only when that creature shares a color with the mana the land made (a red creature
//! for a Mountain's {R}, not a green one), another land tapped for mana triggers nothing, and each game replays to its
//! live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_cards::value::CounterKind;
use mtg_core::color::Color;
use mtg_core::ids::ObjectId;
use mtg_engine::replay;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{creature, forest, mountain};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, attached_to, ready};
use rstest::rstest;

/// Seat 0 controls a Mountain fortified by C.A.M.P., a Forest, a red and a green creature, in its first main phase.
fn table(seats: usize) -> Driver {
    let scenario = (0..seats).fold(
        Scenario::new(seats).at(1, 0, Step::Main1).battlefield(
            0,
            [
                ready(mountain()),
                attached_to(ready(card("C.A.M.P.").unwrap()), 0, 0),
                ready(forest()),
                ready(creature("Red Scout", 0, &[Color::Red], 1, 1)),
                ready(creature("Green Scout", 0, &[Color::Green], 1, 1)),
            ],
        ),
        |scenario, seat| scenario.library(seat, vec![mountain(); 8]),
    );
    Driver::from_scenario(scenario).unwrap()
}

fn counters(driver: &Driver, id: ObjectId) -> u32 {
    driver
        .state()
        .object(id)
        .unwrap()
        .permanent()
        .unwrap()
        .counters_of(&CounterKind::PlusOnePlusOne)
}

fn junk(driver: &Driver) -> usize {
    driver
        .state()
        .zone(Zone::Battlefield)
        .iter()
        .filter(|id| driver.state().object(**id).unwrap().card().name == "Junk")
        .count()
}

/// Resolves the trigger with `target` as its target.
fn resolve(driver: &mut Driver, target: &str) {
    let aim = driver.target_named(target).unwrap();
    driver.choose_targets(&[aim]).unwrap();
    while !driver.state().stack().is_empty() {
        driver.pass().unwrap();
    }
}

#[rstest]
#[case::a_red_creature_shares_the_red_mana("Red Scout", 1)]
#[case::a_green_creature_does_not("Green Scout", 0)]
fn camp_counts_and_makes_junk_by_color(
    #[values(2, 3, 4)] seats: usize,
    #[case] target: &str,
    #[case] made: usize,
) {
    let mut driver = table(seats);
    let id = driver.id_of(target).unwrap();
    driver.tap_for_mana("Mountain").unwrap();
    resolve(&mut driver, target);
    assert_eq!(counters(&driver, id), 1);
    assert_eq!(junk(&driver), made);
    let state = replay(driver.engine().setup(), driver.engine().events()).unwrap();
    assert_eq!(state.state_hash(), driver.state().state_hash());
}

#[rstest]
fn camp_ignores_an_unfortified_land(#[values(2, 4)] seats: usize) {
    let mut driver = table(seats);
    driver.tap_for_mana("Forest").unwrap();
    assert_eq!(driver.state().stack().len(), 0, "nothing triggers");
    assert!(
        !matches!(
            driver.decision().unwrap().kind,
            mtg_engine::decision::DecisionKind::ChooseTargets(_)
        ),
        "nothing triggers"
    );
}
