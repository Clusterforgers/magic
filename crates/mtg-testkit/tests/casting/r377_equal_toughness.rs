//! Goal: "Choose any number of target creatures with equal toughness" (V.A.T.S.) takes targets of one toughness only,
//! across players and with none at all, refuses a set of two toughnesses, and destroys every chosen creature, at two
//! to four seats; each log replays.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_engine::replay;
use mtg_engine::setup::PresetPermanent;
use mtg_engine::turn::Step;
use mtg_testkit::cards::{free_creature, swamp};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ready};
use rstest::rstest;

#[rstest]
fn r377_vats_destroys_creatures_of_one_toughness(
    #[values(2, 3, 4)] seats: usize,
    #[values(&["Two A", "Two B"][..], &["Two A", "Three"][..], &[][..], &["Three"][..], &["Two A", "Two B", "Two C"][..])]
    chosen: &[&str],
) {
    let mut theirs = vec![
        ready(free_creature("Two A", 1, 2)),
        ready(free_creature("Two B", 4, 2)),
        ready(free_creature("Three", 2, 3)),
    ];
    let two_c = ready(free_creature("Two C", 5, 2));
    let mut scenario = Scenario::new(seats)
        .at(3, 0, Step::Main1)
        .battlefield(
            0,
            (0..4)
                .map(|_| PresetPermanent::new(swamp()))
                .collect::<Vec<_>>(),
        )
        .hand(0, [card("V.A.T.S.").unwrap()]);
    if seats == 2 {
        theirs.push(two_c);
    } else {
        scenario = scenario.battlefield(seats - 1, [two_c]);
    }
    let scenario = scenario.battlefield(1, theirs);
    let mut driver = Driver::from_scenario(scenario.clone()).unwrap();
    driver.pass_until(Step::Main1).unwrap();
    let targets: Vec<_> = chosen
        .iter()
        .map(|name| driver.target_named(name).unwrap())
        .collect();
    let alike = chosen.iter().all(|name| name.starts_with("Two")) || chosen.len() < 2;
    let cast = driver.cast_at("V.A.T.S.", &targets);
    if !alike {
        assert!(cast.is_err(), "two toughnesses: {chosen:?}");
        return;
    }
    cast.unwrap();
    while !driver.state().stack().is_empty() {
        driver.pass().unwrap();
    }
    for name in ["Two A", "Two B", "Two C", "Three"] {
        assert_eq!(
            driver.target_named(name).is_err(),
            chosen.contains(&name),
            "{name}"
        );
    }
    let setup = scenario.setup().unwrap();
    let replayed = replay(&setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.engine().state_hash());
}
