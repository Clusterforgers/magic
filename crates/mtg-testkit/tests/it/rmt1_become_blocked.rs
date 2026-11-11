//! Goal: Fog Patch plays at tables of two to four seats as its rules say: it is cast only during the declare blockers step, every attacking creature becomes blocked with no creature blocking it, a blocked creature deals no combat damage unless it has trample (then all of it to the player it attacks), and "becomes blocked" triggers (afflict) fire for each.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common::pid;
use mtg_cards::keyword::Keyword;
use mtg_engine::decision::DecisionKind;
use mtg_engine::turn::Step;
use mtg_testkit::cards::{forest, free_creature, keyworded_creature};
use mtg_testkit::corpus::corpus;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// Seat 0 controls a 3/3 Raider, a 0/3 Afflicter with afflict 2 and a 3/3 Trampler, ready; the last seat holds Fog Patch
/// and two Forests.
fn rmt1_table(seats: usize) -> Scenario {
    let fog = corpus().unwrap().card("Fog Patch").unwrap();
    let defender = seats - 1;
    Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .battlefield(
            0,
            [
                ready(free_creature("Raider", 3, 3)),
                ready(keyworded_creature(
                    "Afflicter",
                    0,
                    &[],
                    0,
                    3,
                    &[Keyword::Afflict(2)],
                )),
                ready(keyworded_creature(
                    "Trampler",
                    0,
                    &[],
                    3,
                    3,
                    &[Keyword::Trample],
                )),
            ],
        )
        .battlefield(defender, [ready(forest()), ready(forest())])
        .hand(defender, [fog])
}

fn rmt1_life(driver: &Driver, seat: usize) -> i32 {
    driver.state().player(pid(seat)).unwrap().life()
}

/// Seat 0 attacks the last seat with everything; nobody blocks; the last seat casts Fog Patch when it first has priority
/// in the declare blockers step (when `fog`), and the game goes on to the second main phase.
fn rmt1_fight(driver: &mut Driver, seats: usize, fog: bool) -> Result<(), ScriptError> {
    let defender = seats - 1;
    driver.pass_until(Step::DeclareAttackers)?;
    driver.attack(&[
        ("Raider", defender),
        ("Afflicter", defender),
        ("Trampler", defender),
    ])?;
    let mut cast = !fog;
    for _ in 0..80 {
        let decision = driver.decision()?.clone();
        let step = driver.state().turn().step();
        match &decision.kind {
            DecisionKind::DeclareBlockers(_) => {
                driver.block(&[])?;
            }
            DecisionKind::Priority(_)
                if !cast && step == Step::DeclareBlockers && decision.player == pid(defender) =>
            {
                driver.cast_tapping("Fog Patch", &["Forest", "Forest"])?;
                cast = true;
            }
            DecisionKind::Priority(_)
                if step == Step::Main2 && driver.state().stack().is_empty() =>
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
    panic!("the game did not reach the second main phase");
}

#[rstest]
fn rmt1_attackers_that_become_blocked_deal_damage_only_with_trample(
    #[values(2, 3, 4)] seats: usize,
    #[values(true, false)] fog: bool,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(rmt1_table(seats))?;
    rmt1_fight(&mut driver, seats, fog)?;
    let defender = seats - 1;
    // Unblocked: the Raider and the Trampler deal 3 each. Blocked by Fog Patch: afflict costs 2 and the Trampler alone
    // deals its 3.
    let lost = if fog { 2 + 3 } else { 3 + 3 };
    assert_eq!(rmt1_life(&driver, defender), 20 - lost);
    for seat in 1..defender {
        assert_eq!(rmt1_life(&driver, seat), 20, "seat {seat} was not attacked");
    }
    Ok(())
}

#[rstest]
fn rmt1_fog_patch_is_not_cast_outside_the_declare_blockers_step(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(rmt1_table(seats))?;
    let defender = seats - 1;
    while driver.decision()?.player != pid(defender) {
        driver.pass()?;
    }
    assert!(
        driver
            .cast_tapping("Fog Patch", &["Forest", "Forest"])
            .is_err()
    );
    Ok(())
}
