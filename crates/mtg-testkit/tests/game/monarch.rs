//! Goal: the monarch (CR 724) works at tables of two, three and four seats: a spell makes its controller the only monarch and a second time changes nothing; the monarch draws a card at the beginning of their own end step and no other seat's; a creature that deals combat damage to the monarch passes the designation to its controller, from the first player in turn order when several hit; blocked damage or damage to another player changes nothing; a monarch who loses the game gives the designation to the active player; and the log replays to the state it was played to.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use common::combat::to_blockers;
use common::pid;
use mtg_core::ids::PlayerId;
use mtg_engine::turn::Step;
use mtg_engine::{Event, replay};
use mtg_testkit::cards::{free_creature, island, monarcher};
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// Seat 0 holds a Crown and each other seat a Crown of its own, every seat with an Island; seat 1 has a Raider ready
/// and, with three or more seats, seat 2 a Brute; turn 1 is seat 0's, at its first main step.
fn table(seats: usize) -> Scenario {
    let mut scenario = Scenario::new(seats);
    for seat in 0..seats {
        let mut permanents = vec![ready(island())];
        match seat {
            0 => permanents.push(ready(free_creature("Wall", 0, 5))),
            1 => permanents.push(ready(free_creature("Raider", 2, 2))),
            2 => permanents.push(ready(free_creature("Brute", 3, 3))),
            _ => {}
        }
        scenario = scenario
            .hand(seat, [monarcher(&format!("Crown {seat}"))])
            .battlefield(seat, permanents);
    }
    scenario
}

fn monarch(driver: &Driver) -> Option<usize> {
    driver.state().monarch().map(PlayerId::index)
}

/// Casts seat `seat`'s Crown and lets it resolve.
fn crown(driver: &mut Driver, seat: usize) -> Result<(), ScriptError> {
    driver.cast(&format!("Crown {seat}"))?;
    while !driver.state().stack().is_empty() {
        driver.pass()?;
    }
    Ok(())
}

fn changes(driver: &Driver) -> Vec<usize> {
    driver
        .engine()
        .events()
        .iter()
        .filter_map(|event| match event {
            Event::MonarchChanged { player } => Some(player.index()),
            _ => None,
        })
        .collect()
}

fn draws(driver: &Driver, seat: usize) -> usize {
    driver
        .engine()
        .events()
        .iter()
        .filter(|event| matches!(event, Event::CardDrawn { player, .. } if *player == pid(seat)))
        .count()
}

#[rstest]
fn becoming_the_monarch_takes_the_designation_from_the_monarch_and_a_second_time_changes_nothing(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table(seats))?;
    assert_eq!(monarch(&driver), None);
    crown(&mut driver, 0)?;
    assert_eq!(monarch(&driver), Some(0));
    // Seat 1 has the next turn, casts its own Crown and takes the designation.
    driver.idle_until(2, Step::Main1)?;
    crown(&mut driver, 1)?;
    assert_eq!(monarch(&driver), Some(1));
    assert_eq!(changes(&driver), [0, 1]);
    Ok(())
}

#[rstest]
fn the_monarch_draws_a_card_at_the_beginning_of_their_own_end_step_and_at_no_other_seats(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table(seats))?;
    crown(&mut driver, 0)?;
    let before = driver
        .state()
        .zone(mtg_engine::zones::Zone::Hand(pid(0)))
        .len();
    driver.idle_until(1, Step::End)?;
    assert_eq!(draws(&driver, 0), 1, "the monarch drew at their end step");
    assert_eq!(
        driver
            .state()
            .zone(mtg_engine::zones::Zone::Hand(pid(0)))
            .len(),
        before + 1
    );
    // The next turn is seat 1's: its end step gives seat 0 nothing.
    driver.idle_until(2, Step::End)?;
    assert_eq!(draws(&driver, 0), 1);
    Ok(())
}

#[rstest]
fn nobody_draws_at_the_end_step_while_nobody_is_the_monarch(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table(seats))?;
    driver.idle_until(1, Step::End)?;
    assert_eq!(draws(&driver, 0), 0);
    assert_eq!(monarch(&driver), None);
    Ok(())
}

/// The monarch is seat 0; on seat 1's turn its Raider (and with three seats or more seat 2's Brute, which cannot
/// attack out of turn) attacks seat 0.
#[rstest]
#[case::unblocked(false, Some(1))]
#[case::blocked(true, Some(0))]
fn combat_damage_to_the_monarch_passes_the_designation_to_the_attackers_controller(
    #[case] blocked: bool,
    #[case] monarch_after: Option<usize>,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table(seats))?;
    crown(&mut driver, 0)?;
    driver.idle_until(2, Step::DeclareAttackers)?;
    driver.attack(&[("Raider", 0)])?;
    to_blockers(&mut driver, 1)?;
    driver.block(&if blocked {
        vec![("Wall", "Raider")]
    } else {
        vec![]
    })?;
    driver.idle_until(2, Step::EndCombat)?;
    assert_eq!(monarch(&driver), monarch_after);
    Ok(())
}

#[rstest]
fn damage_to_a_player_who_is_not_the_monarch_changes_nothing(
    #[values(3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table(seats))?;
    crown(&mut driver, 0)?;
    driver.idle_until(2, Step::DeclareAttackers)?;
    driver.attack(&[("Raider", 2)])?;
    to_blockers(&mut driver, 1)?;
    driver.block(&[])?;
    driver.idle_until(2, Step::EndCombat)?;
    assert_eq!(monarch(&driver), Some(0), "the damage went to seat 2");
    Ok(())
}

#[rstest]
fn a_monarch_who_loses_the_game_gives_the_designation_to_the_active_player(
    #[values(2, 3)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table(seats))?;
    crown(&mut driver, 0)?;
    driver.idle_until(2, Step::Main1)?;
    assert_eq!(monarch(&driver), Some(0));
    driver.concede(pid(0))?;
    assert_eq!(monarch(&driver), Some(1), "seat 1 has the turn");
    Ok(())
}

#[rstest]
fn the_log_of_a_game_with_a_monarch_replays_to_the_state_it_was_played_to(
    #[values(2, 3)] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = table(seats);
    let setup = scenario.clone().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario)?;
    crown(&mut driver, 0)?;
    driver.idle_until(2, Step::DeclareAttackers)?;
    driver.attack(&[("Raider", 0)])?;
    to_blockers(&mut driver, 1)?;
    driver.block(&[])?;
    driver.idle_until(2, Step::End)?;
    assert_eq!(monarch(&driver), Some(1));
    let replayed = replay(&setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.engine().state_hash());
    assert_eq!(replayed.monarch(), driver.state().monarch());
    Ok(())
}
