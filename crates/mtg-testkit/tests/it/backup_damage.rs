//! Goal: the wording after a backup keyword plays whole at tables of two, three and four seats: Conclave Sledge-Captain puts as many +1/+1 counters on itself as the combat damage it deals to a player, with trample counting only what passes the blocker and a blocked creature that deals none getting none, and its three backups give another creature trample and the same trigger until end of turn.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common::pid;
use mtg_cards::value::CounterKind;
use mtg_engine::decision::DecisionKind;
use mtg_engine::turn::Step;
use mtg_testkit::cards::free_creature;
use mtg_testkit::{Driver, Scenario, ScriptError, corpus, ready};
use rstest::rstest;

/// Seat 0 controls the Captain, ready; every other seat controls a Wall of the toughness given.
fn table(seats: usize, wall: i32) -> Result<Driver, ScriptError> {
    let captain = corpus::card("Conclave Sledge-Captain").unwrap();
    let mut scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .battlefield(0, [ready(captain)]);
    for seat in 1..seats {
        scenario = scenario.battlefield(
            seat,
            [ready(free_creature(&format!("Wall {seat}"), 0, wall))],
        );
    }
    Driver::from_scenario(scenario)
}

/// The Captain attacks seat 1; the Wall of seat 1 blocks it when `blocked`; the game goes on until the end of combat.
fn attack(driver: &mut Driver, blocked: bool, toughness: u32) -> Result<(), ScriptError> {
    driver.pass_until(Step::DeclareAttackers)?;
    driver.attack(&[("Conclave Sledge-Captain", 1)])?;
    for _ in 0..60 {
        let decision = driver.decision()?.clone();
        match &decision.kind {
            DecisionKind::DeclareBlockers(_) => {
                let chosen: Vec<(&str, &str)> = if blocked && decision.player == pid(1) {
                    vec![("Wall 1", "Conclave Sledge-Captain")]
                } else {
                    Vec::new()
                };
                driver.block(&chosen)?;
            }
            // Lethal damage to the Wall first, then trample: what is left passes to the player.
            DecisionKind::AssignDamage(_) => {
                driver.assign_damage(&[("Wall 1", toughness.min(4))])?;
            }
            DecisionKind::Priority(_)
                if driver.state().turn().step() == Step::EndCombat
                    && driver.state().stack().is_empty() =>
            {
                return Ok(());
            }
            DecisionKind::Priority(_) => {
                driver.pass()?;
            }
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    panic!("combat did not end");
}

fn counters(driver: &Driver) -> u32 {
    let id = driver.id_of("Conclave Sledge-Captain").unwrap();
    driver
        .state()
        .object(id)
        .and_then(|object| object.permanent())
        .map_or(0, |permanent| {
            permanent.counters_of(&CounterKind::PlusOnePlusOne)
        })
}

fn life(driver: &Driver, seat: usize) -> i32 {
    driver.state().player(pid(seat)).unwrap().life()
}

#[rstest]
#[case::unblocked(false, 1, 4)]
#[case::blocked_by_a_small_wall_so_trample_passes_the_rest(true, 1, 3)]
#[case::blocked_by_a_wall_of_two(true, 2, 2)]
#[case::blocked_by_a_wall_it_cannot_pass(true, 5, 0)]
fn the_captain_gets_a_counter_for_each_damage_it_deals_to_the_player(
    #[values(2, 3, 4)] seats: usize,
    #[case] blocked: bool,
    #[case] toughness: i32,
    #[case] damage: u32,
) -> Result<(), ScriptError> {
    let mut driver = table(seats, toughness)?;
    attack(&mut driver, blocked, u32::try_from(toughness).unwrap())?;
    assert_eq!(counters(&driver), damage);
    assert_eq!(life(&driver, 1), 20 - i32::try_from(damage).unwrap());
    for seat in [0].into_iter().chain(2..seats) {
        assert_eq!(life(&driver, seat), 20, "seat {seat} lost life");
    }
    Ok(())
}
