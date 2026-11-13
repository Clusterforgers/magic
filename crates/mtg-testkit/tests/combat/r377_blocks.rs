//! Goal: "can't be blocked unless all creatures defending player controls block it" (Tromokratis) admits a block only by
//! every creature the defending player controls and none while one of them can't block, whatever other players
//! control, and "lethal damage dealt to creatures you control is determined by their power" (Zilortha) destroys its
//! controller's creature with damage at least its power (some damage at power 0) and no one else's, at two to four
//! seats; each log replays.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_engine::replay;
use mtg_engine::setup::PresetPermanent;
use mtg_engine::turn::Step;
use mtg_testkit::cards::free_creature;
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ready};
use rstest::rstest;

/// The log of the driver replays to its state.
fn replays(scenario: &Scenario, driver: &Driver) {
    let setup = scenario.clone().setup().unwrap();
    let replayed = replay(&setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.engine().state_hash());
}

const DEFENDERS: [&str; 3] = ["D1", "D2", "D3"];

#[rstest]
fn r377_tromokratis_is_blocked_by_all_or_none(
    #[values(2, 3, 4)] seats: usize,
    #[values(1, 2, 3)] defenders: usize,
    #[values(false, true)] one_tapped: bool,
    #[values(0, 1)] left_out: usize,
) {
    let mut side: Vec<PresetPermanent> = DEFENDERS[..defenders]
        .iter()
        .map(|name| ready(free_creature(name, 1, 1)))
        .collect();
    if one_tapped {
        let mut tapped = ready(free_creature("Sleeper", 1, 1));
        tapped.tapped = true;
        side.push(tapped);
    }
    let mut scenario = Scenario::new(seats)
        .at(2, 0, Step::DeclareAttackers)
        .battlefield(0, [ready(card("Tromokratis").unwrap())])
        .battlefield(1, side);
    if seats > 2 {
        scenario = scenario.battlefield(2, [ready(free_creature("Bystander", 1, 1))]);
    }
    let mut driver = Driver::from_scenario(scenario.clone()).unwrap();
    driver.attack(&[("Tromokratis", 1)]).unwrap();
    driver.pass_until(Step::DeclareBlockers).unwrap();
    let used = defenders - left_out.min(defenders - 1);
    let blocks: Vec<(&str, &str)> = DEFENDERS[..used]
        .iter()
        .map(|name| (*name, "Tromokratis"))
        .collect();
    let answer = driver.block(&blocks);
    if one_tapped || used < defenders {
        assert!(
            answer.is_err(),
            "{used} of {defenders} blockers, a tapped creature: {one_tapped}"
        );
        return;
    }
    answer.unwrap();
    replays(&scenario, &driver);
}

#[rstest]
fn r377_zilortha_judges_lethal_damage_by_power(
    #[values(2, 3, 4)] seats: usize,
    #[values(false, true)] zilortha: bool,
    #[values((2, 5, 3, true), (3, 5, 2, false), (0, 4, 1, true))] fight: (i32, i32, i32, bool),
) {
    let (power, toughness, blocker_power, dies_by_power) = fight;
    let mut mine = vec![ready(free_creature("Tank", power, toughness))];
    if zilortha {
        mine.push(ready(card("Zilortha, Strength Incarnate").unwrap()));
    }
    let scenario = Scenario::new(seats)
        .at(2, 0, Step::DeclareAttackers)
        .battlefield(0, mine)
        .battlefield(1, [ready(free_creature("Biter", blocker_power, 9))]);
    let mut driver = Driver::from_scenario(scenario.clone()).unwrap();
    driver.attack(&[("Tank", 1)]).unwrap();
    driver.pass_until(Step::DeclareBlockers).unwrap();
    driver.block(&[("Biter", "Tank")]).unwrap();
    driver.pass_until(Step::EndCombat).unwrap();
    assert_eq!(
        driver.target_named("Tank").is_err(),
        zilortha && dies_by_power,
        "a {power}/{toughness} dealt {blocker_power}"
    );
    assert!(
        driver.target_named("Biter").is_ok(),
        "the opponent's creature keeps its toughness"
    );
    replays(&scenario, &driver);
}
