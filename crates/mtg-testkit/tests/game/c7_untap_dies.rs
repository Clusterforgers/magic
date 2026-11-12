//! Goal: "it fights up to one target creature an opponent controls. If that creature would die this turn, exile it instead" (Mawloc) exiles the fought creature that dies of the fight and leaves every other death alone (CR 614.1a, 611.2c): Mawloc cast with X 0 dies with it and goes to the graveyard, with X 1 it survives, at tables of two, three and four seats with the creature on the next seat or the last one; the log replays to the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use common::{names, pid};
use mtg_engine::Answer;
use mtg_engine::choose::{NumberChoice, Target};
use mtg_engine::decision::DecisionKind;
use mtg_engine::replay;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{forest, mountain};
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// The lands seat 0 taps for Mawloc with X.
fn lands(x: u32) -> Vec<&'static str> {
    let mut lands = vec!["Mountain", "Forest"];
    lands.extend((0..x).map(|_| "Mountain"));
    lands
}

/// Seat 0 holds Mawloc over its lands; `holder` controls Grizzly Bears.
fn game(seats: usize, holder: usize, x: u32) -> Scenario {
    Scenario::new(seats)
        .hand_named(0, ["Mawloc"])
        .battlefield(
            0,
            lands(x).into_iter().map(|name| {
                ready(if name == "Forest" {
                    forest()
                } else {
                    mountain()
                })
            }),
        )
        .battlefield_named(holder, ["Grizzly Bears"])
}

/// Passes until a player has priority over an empty stack, choosing the Bears for the trigger of Mawloc.
fn settle(driver: &mut Driver, bears: Target) -> Result<(), ScriptError> {
    for _ in 0..200 {
        let decision = driver.engine().decision().map(|d| d.kind.clone());
        let idle = driver.state().stack().is_empty() && driver.state().resolving().is_none();
        match decision {
            Some(DecisionKind::Priority(_)) if idle => return Ok(()),
            Some(DecisionKind::Priority(_)) => {
                driver.pass()?;
            }
            Some(DecisionKind::ChooseTargets(_)) => {
                driver.choose_targets(std::slice::from_ref(&bears))?;
            }
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    Err(ScriptError::Stuck("the stack".into()))
}

#[rstest]
fn the_fought_creature_is_exiled_instead_of_dying(
    #[values(2, 3, 4)] seats: usize,
    #[values(true, false)] next_seat: bool,
    #[values(0, 1)] x: u32,
) -> Result<(), ScriptError> {
    let holder = if next_seat { 1 } else { seats - 1 };
    let setup = game(seats, holder, x).setup().unwrap();
    let mut driver = Driver::from_scenario(game(seats, holder, x))?;
    driver.pass_until(Step::Main1)?;
    let bears = driver.id_of("Grizzly Bears").unwrap();
    let bears = Target::Object(driver.state().object(bears).unwrap().reference());
    driver.begin_cast("Mawloc")?;
    while matches!(driver.decision()?.kind, DecisionKind::ChooseNumber(_)) {
        driver.answer(Answer::ChooseNumber(NumberChoice::Number(x)))?;
    }
    driver.pay_tapping(&lands(x), 0)?;
    settle(&mut driver, bears)?;
    assert_eq!(names(driver.state(), Zone::Exile), ["Grizzly Bears"]);
    assert_eq!(
        names(driver.state(), Zone::Graveyard(pid(holder))),
        Vec::<String>::new()
    );
    let dead: Vec<String> = if x == 0 {
        vec!["Mawloc".to_owned()]
    } else {
        Vec::new()
    };
    assert_eq!(
        names(driver.state(), Zone::Graveyard(pid(0))),
        dead,
        "Mawloc itself dies as usual"
    );
    let replayed = replay(&setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
    Ok(())
}
