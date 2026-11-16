//! Goal: a Delves hero casts the cards of their Satchel from the command zone at two to four seats, at the timing of
//! each card's type like a card in hand: a creature at sorcery speed in their own main phase and never in another
//! player's turn, an instant in another player's turn; a cast card leaves the Satchel for good, and a permanent cast
//! from it that dies goes to its owner's graveyard.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use common::pid;
use mtg_core::color::Color;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{burn, creature, forest, mountain};
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// Seat 0 holds a Grizzly ({1}{G}, 2/2) and a Bolt (3 damage for {R}) in its Satchel, a Shock in hand, and two
/// Forests and two Mountains; every other seat has an empty board.
fn table(seats: usize) -> Scenario {
    Scenario::new(seats)
        .satchel(
            0,
            [
                creature("Grizzly", 1, &[Color::Green], 2, 2),
                burn("Bolt", 3),
            ],
        )
        .hand(0, [burn("Shock", 2)])
        .battlefield(
            0,
            [
                ready(forest()),
                ready(forest()),
                ready(mountain()),
                ready(mountain()),
            ],
        )
}

fn satchel(driver: &Driver) -> Vec<String> {
    driver
        .state()
        .satchel(pid(0))
        .into_iter()
        .map(|id| driver.state().object(id).unwrap().card().name.clone())
        .collect()
}

/// Passes until seat 0 holds priority.
fn until_seat_zero(driver: &mut Driver) -> Result<(), ScriptError> {
    while driver.decision()?.player != pid(0) {
        driver.pass()?;
    }
    Ok(())
}

fn resolve_stack(driver: &mut Driver) -> Result<(), ScriptError> {
    while !driver.state().stack().is_empty() {
        driver.pass()?;
    }
    Ok(())
}

#[rstest]
fn a_creature_is_cast_from_the_satchel_in_its_owners_main_phase(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table(seats))?;
    driver.idle_until(1, Step::Main1)?;
    assert_eq!(satchel(&driver), ["Grizzly", "Bolt"]);
    driver.cast("Grizzly")?;
    resolve_stack(&mut driver)?;
    let grizzly = driver.id_of("Grizzly")?;
    assert_eq!(
        driver.state().object(grizzly).unwrap().zone(),
        Zone::Battlefield
    );
    assert_eq!(satchel(&driver), ["Bolt"]);
    Ok(())
}

#[rstest]
fn an_instant_is_cast_from_the_satchel_in_another_players_turn(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table(seats))?;
    driver.idle_until(2, Step::Main1)?;
    until_seat_zero(&mut driver)?;
    let target = driver.target_player(1);
    driver.cast_at("Bolt", &[target])?;
    resolve_stack(&mut driver)?;
    assert_eq!(driver.state().player(pid(1)).unwrap().life(), 17);
    assert_eq!(satchel(&driver), ["Grizzly"]);
    Ok(())
}

#[rstest]
fn a_creature_in_the_satchel_waits_for_its_owners_main_phase(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table(seats))?;
    driver.idle_until(2, Step::Main1)?;
    until_seat_zero(&mut driver)?;
    assert!(driver.begin_cast("Grizzly").is_err());
    assert_eq!(satchel(&driver), ["Grizzly", "Bolt"]);
    Ok(())
}

#[rstest]
fn a_permanent_cast_from_the_satchel_dies_to_the_graveyard(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table(seats))?;
    driver.idle_until(1, Step::Main1)?;
    driver.cast("Grizzly")?;
    resolve_stack(&mut driver)?;
    let grizzly = driver.id_of("Grizzly")?;
    let target = driver.target_named("Grizzly")?;
    driver.cast_at("Shock", &[target])?;
    resolve_stack(&mut driver)?;
    assert_eq!(
        driver.state().object(grizzly).unwrap().zone(),
        Zone::Graveyard(pid(0))
    );
    assert_eq!(satchel(&driver), ["Bolt"]);
    Ok(())
}

#[rstest]
fn an_opponent_cannot_cast_from_anothers_satchel(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver =
        Driver::from_scenario(table(seats).hand(1, []).battlefield(1, [ready(mountain())]))?;
    driver.idle_until(2, Step::Main1)?;
    assert_eq!(driver.decision()?.player, pid(1));
    assert!(driver.begin_cast("Bolt").is_err());
    Ok(())
}

#[rstest]
fn a_bot_casts_its_creature_from_the_satchel(
    #[values(2, 3, 4)] seats: usize,
    #[values(mtg_testkit::brain::Level::Normal, mtg_testkit::brain::Level::Hard)]
    level: mtg_testkit::brain::Level,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table(seats))?;
    driver.idle_until(1, Step::Main1)?;
    let mut bot = mtg_testkit::brain::brain(level, 7, pid(0), false);
    // The bot plays its main phase until it passes.
    while driver.state().turn().step() == Step::Main1 && driver.state().turn().number() == 1 {
        let decision = driver.decision()?.clone();
        let answer = if decision.player == pid(0) {
            bot.choose(&decision, Some(driver.engine()))
        } else {
            Driver::passive_answer(&decision)
        };
        driver.answer(answer)?;
    }
    assert!(!satchel(&driver).contains(&"Grizzly".to_owned()));
    Ok(())
}
