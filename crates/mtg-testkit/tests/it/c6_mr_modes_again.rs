//! Goal: a mode chosen two or three times whose steps bind a name and read it later ("Target player returns a creature card from their graveyard to their hand", Obscura Confluence; "You may put a land card from your hand or graveyard onto the battlefield tapped", Riveteers Confluence) runs once for each time it was chosen at tables of two, three and four seats: each run binds its own choice, every run moves its card, and the log replays to the state it was played to.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_engine::choose::{ModeChoice, Target, YesNoChoice};
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::replay;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{forest, island, mountain, plains, swamp};
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::{names, pid};

/// Seat 0 has the spell in hand, two of each basic land ready, `hand` beside the spell and `graveyard` in its graveyard,
/// in the first main phase of turn 1; seat 1 has a Grizzly Bears.
fn table(
    seats: usize,
    spell: &str,
    hand: &[&str],
    graveyard: &[&str],
) -> Result<Driver, ScriptError> {
    let lands = [forest(), plains(), island(), swamp(), mountain()];
    let mut scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .hand_named(0, [spell].into_iter().chain(hand.iter().copied()))
        .graveyard_named(0, graveyard.iter().copied())
        .battlefield(
            0,
            lands
                .iter()
                .chain(lands.iter())
                .map(|land| ready(land.clone())),
        )
        .battlefield_named(1, ["Grizzly Bears"])
        .library(0, (0..8).map(|_| island()));
    for seat in 1..seats {
        scenario = scenario.library(seat, (0..8).map(|_| island()));
    }
    Driver::from_scenario(scenario)
}

/// Casts the spell with the modes, each target seat 0 when it may be and else the first legal one, and plays until the
/// stack is empty, saying yes to every "may".
fn cast(driver: &mut Driver, spell: &str, modes: &[usize]) -> Result<(), ScriptError> {
    driver.begin_cast(spell)?;
    driver.answer(Answer::ChooseMode(ModeChoice::Modes(modes.to_vec())))?;
    while let DecisionKind::ChooseTargets(options) = &driver.decision()?.kind {
        let me = driver.target_player(0);
        let target: Target = if options.legal.contains(&me) {
            me
        } else {
            options.legal[0]
        };
        driver.choose_targets(&[target])?;
    }
    driver.answer(Answer::PayMana(PayChoice::Auto))?;
    for _ in 0..200 {
        if driver.state().stack().is_empty() && driver.state().resolving().is_none() {
            return Ok(());
        }
        if matches!(driver.decision()?.kind, DecisionKind::YesNo(_)) {
            driver.answer(Answer::YesNo(YesNoChoice(true)))?;
        } else {
            driver.answer_passively()?;
        }
    }
    Err(ScriptError::Stuck(spell.into()))
}

/// How many cards of the name are in the zone.
fn count(driver: &Driver, zone: Zone, name: &str) -> usize {
    names(driver.state(), zone)
        .iter()
        .filter(|card| *card == name)
        .count()
}

#[rstest]
#[case::returned_twice_after_a_connive(&[1, 2, 2], 2)]
#[case::returned_three_times(&[2, 2, 2], 3)]
fn each_run_of_a_return_mode_returns_its_own_card(
    #[values(2, 3, 4)] seats: usize,
    #[case] modes: &[usize],
    #[case] returned: usize,
) -> Result<(), ScriptError> {
    let spell = "Obscura Confluence";
    let mut driver = table(seats, spell, &[], &["Grizzly Bears"; 3])?;
    cast(&mut driver, spell, modes)?;
    assert_eq!(
        count(&driver, Zone::Hand(pid(0)), "Grizzly Bears"),
        returned
    );
    assert_eq!(
        count(&driver, Zone::Graveyard(pid(0)), "Grizzly Bears"),
        3 - returned
    );
    let copy = replay(driver.engine().setup(), driver.engine().events())?;
    assert_eq!(copy.state_hash(), driver.engine().state_hash());
    Ok(())
}

#[rstest]
#[case::put_twice_after_a_draw(&[0, 2, 2], 2)]
#[case::put_three_times(&[2, 2, 2], 3)]
fn each_run_of_a_land_mode_puts_its_own_land(
    #[values(2, 3, 4)] seats: usize,
    #[case] modes: &[usize],
    #[case] put: usize,
) -> Result<(), ScriptError> {
    let spell = "Riveteers Confluence";
    let mut driver = table(seats, spell, &["Forest"; 3], &[])?;
    let before = names(driver.state(), Zone::Battlefield).len();
    cast(&mut driver, spell, modes)?;
    assert_eq!(names(driver.state(), Zone::Battlefield).len(), before + put);
    let copy = replay(driver.engine().setup(), driver.engine().events())?;
    assert_eq!(copy.state_hash(), driver.engine().state_hash());
    Ok(())
}
