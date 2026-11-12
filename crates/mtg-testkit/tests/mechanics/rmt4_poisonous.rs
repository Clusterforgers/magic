//! Goal: poisonous N (a keyword macro) gives the player a creature with it deals combat damage to N poison counters, at tables of two, three and four seats: Virulent Sliver grants poisonous 1 to Slivers only, Snake Cult Initiation poisonous 3 to the creature it enchants; a creature without the keyword gives none; every game replays to the same state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_cards::value::CounterKind;
use mtg_engine::turn::Step;
use mtg_testkit::cards::free_creature;
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, attached_to, ready};
use rstest::rstest;

use crate::c4_control::{replays, settle};
use crate::common::pid;

fn poison(driver: &Driver, seat: usize) -> u32 {
    driver
        .state()
        .player(pid(seat))
        .unwrap()
        .counters_of(&CounterKind::Poison)
}

/// Seat 0's `attacker` attacks seat 1 unblocked and deals its combat damage; the game is then in seat 0's second main phase.
fn struck(scenario: Scenario, attacker: &str) -> Result<Driver, ScriptError> {
    let mut driver = Driver::from_scenario(scenario)?;
    driver.pass_until(Step::DeclareAttackers)?;
    driver.attack(&[(attacker, 1)])?;
    driver.pass_until(Step::Main2)?;
    settle(&mut driver, None)?;
    Ok(driver)
}

#[rstest]
#[case::the_sliver_has_poisonous_1("Virulent Sliver", 1)]
#[case::a_bear_has_none("Bear", 0)]
fn virulent_sliver_poisons_with_slivers(
    #[case] attacker: &str,
    #[case] counters: u32,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = Scenario::new(seats).at(1, 0, Step::Main1).battlefield(
        0,
        [
            ready(card("Virulent Sliver").unwrap()),
            ready(free_creature("Bear", 2, 2)),
        ],
    );
    let driver = struck(scenario, attacker)?;
    assert_eq!(poison(&driver, 1), counters);
    for seat in 2..seats {
        assert_eq!(poison(&driver, seat), 0);
    }
    replays(&driver);
    Ok(())
}

#[rstest]
#[case::the_enchanted_creature("Bear", 3)]
#[case::another_creature("Wolf", 0)]
fn snake_cult_initiation_poisons_with_the_enchanted_creature(
    #[case] attacker: &str,
    #[case] counters: u32,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = Scenario::new(seats).at(1, 0, Step::Main1).battlefield(
        0,
        [
            ready(free_creature("Bear", 2, 2)),
            ready(free_creature("Wolf", 2, 2)),
            attached_to(ready(card("Snake Cult Initiation").unwrap()), 0, 0),
        ],
    );
    let driver = struck(scenario, attacker)?;
    assert_eq!(poison(&driver, 1), counters);
    replays(&driver);
    Ok(())
}
