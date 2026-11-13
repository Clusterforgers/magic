//! Goal: "if Wolverine dealt damage to another creature this turn" reads the turn's damage as the damaged creature was:
//! Wolverine gets a +1/+1 counter at the end step after it dealt combat damage to a blocker, also a blocker the
//! damage destroyed, and none after it dealt damage only to a player, or did not attack; at two to four seats; the log
//! replays.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_cards::value::CounterKind;
use mtg_engine::decision::DecisionKind;
use mtg_engine::replay;
use mtg_engine::turn::Step;
use mtg_testkit::cards::free_creature;
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

const R5523_WOLVERINE: &str = "Wolverine, Best There Is";

/// Seat 0 begins its turn with Wolverine; seat 1 controls a Wall (0/1) and a Tank (0/20).
fn r5523_table(seats: usize) -> Scenario {
    Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .battlefield(0, [ready(card(R5523_WOLVERINE).unwrap())])
        .battlefield(
            1,
            [
                ready(free_creature("Wall", 0, 1)),
                ready(free_creature("Tank", 0, 20)),
            ],
        )
}

fn r5523_counters(driver: &Driver) -> u32 {
    let id = driver.id_of(R5523_WOLVERINE).unwrap();
    driver
        .state()
        .object(id)
        .unwrap()
        .permanent()
        .unwrap()
        .counters()
        .filter(|(kind, _)| **kind == CounterKind::PlusOnePlusOne)
        .map(|(_, n)| n)
        .sum()
}

#[rstest]
#[case::blocker_survives(Some("Tank"), true)]
#[case::blocker_destroyed(Some("Wall"), true)]
#[case::near_miss_player_only(None, false)]
fn r5523_wolverine_grows_after_damaging_a_creature(
    #[values(2, 3, 4)] seats: usize,
    #[case] blocker: Option<&str>,
    #[case] grows: bool,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(r5523_table(seats))?;
    let mut attacked = false;
    for _ in 0..400 {
        if driver.state().turn().active().index() != 0 {
            break;
        }
        let decision = driver.decision()?.clone();
        match &decision.kind {
            DecisionKind::DeclareAttackers(_) if !attacked => {
                attacked = true;
                driver.attack(&[(R5523_WOLVERINE, 1)])?;
            }
            DecisionKind::DeclareBlockers(_) if decision.player.index() == 1 => {
                match blocker {
                    Some(name) => driver.block(&[(name, R5523_WOLVERINE)])?,
                    None => driver.block(&[])?,
                };
            }
            _ => {
                driver.answer(Driver::passive_answer(&decision))?;
            }
        }
    }
    assert!(attacked, "Wolverine attacked");
    assert_eq!(r5523_counters(&driver), u32::from(grows));
    let setup = r5523_table(seats).setup().unwrap();
    let replayed = replay(&setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.engine().state_hash());
    Ok(())
}

#[rstest]
fn r5523_wolverine_stays_without_combat(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(r5523_table(seats))?;
    for _ in 0..400 {
        if driver.state().turn().active().index() != 0 {
            break;
        }
        let decision = driver.decision()?.clone();
        if matches!(decision.kind, DecisionKind::DeclareAttackers(_)) {
            driver.attack(&[])?;
        } else {
            driver.answer(Driver::passive_answer(&decision))?;
        }
    }
    assert_eq!(r5523_counters(&driver), 0);
    Ok(())
}
