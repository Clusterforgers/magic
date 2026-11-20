//! Goal: control follows the damage and its loss is an event at 2 to 4 seats: damage that would be dealt to Khârn the
//! Betrayer is prevented and an opponent its controller chooses gains control of it (`ReplaceBy::PreventAnd`), and the
//! player who lost control draws two (`Action::R112LostControl`, CR 613.1b, the Khârn ruling), as they do when it
//! leaves the battlefield; each log replays to the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_engine::characteristics::controller_of;
use mtg_engine::choose::NumberChoice;
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{mountain, plains, swamp};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::copying::replays;
use crate::common::pid;

/// Seat 0 controls Khârn, six Mountains and three Swamps in the first main phase, with `spell` in hand; each library holds Plains.
fn table(seats: usize, spell: &str) -> Scenario {
    let mut board = vec![ready(card("Khârn the Betrayer").unwrap())];
    board.extend((0..6).map(|_| ready(mountain())));
    board.extend((0..3).map(|_| ready(swamp())));
    let mut scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .hand_named(0, [spell])
        .battlefield(0, board);
    for seat in 0..seats {
        scenario = scenario.library(seat, (0..8).map(|_| plains()));
    }
    scenario
}

/// Plays on until the stack is empty, answering every question the default way.
fn settle(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..200 {
        if driver.state().stack().is_empty() && driver.state().resolving().is_none() {
            return Ok(());
        }
        if matches!(driver.decision()?.kind, DecisionKind::PayMana(_)) {
            driver.answer(Answer::PayMana(PayChoice::Auto))?;
        } else {
            driver.answer_passively()?;
        }
    }
    Err(ScriptError::Stuck("the stack".into()))
}

#[rstest]
fn damage_to_kharn_gives_it_away_and_its_controller_draws(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = || table(seats, "Blaze");
    let setup = scenario().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario())?;
    driver.begin_cast("Blaze")?;
    driver.answer(Answer::ChooseNumber(NumberChoice::Number(3)))?;
    let kharn = driver.target_named("Khârn the Betrayer")?;
    driver.choose_targets(&[kharn])?;
    driver.answer(Answer::PayMana(PayChoice::Auto))?;
    settle(&mut driver)?;
    let id = driver.id_of("Khârn the Betrayer")?;
    let state = driver.state();
    // The damage was prevented, an opponent controls it and seat 0 drew two.
    assert_eq!(state.object(id).unwrap().permanent().unwrap().damage(), 0);
    let now = controller_of(state, id).unwrap();
    assert_ne!(now, pid(0));
    assert_eq!(state.zone(Zone::Hand(pid(0))).len(), 2);
    assert_eq!(state.zone(Zone::Hand(now)).len(), 0);
    replays(&driver, &setup);
    Ok(())
}

#[rstest]
fn kharn_leaving_the_battlefield_draws_its_controller_two(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = || table(seats, "Murder");
    let setup = scenario().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario())?;
    driver.begin_cast("Murder")?;
    let kharn = driver.target_named("Khârn the Betrayer")?;
    driver.choose_targets(&[kharn])?;
    driver.answer(Answer::PayMana(PayChoice::Auto))?;
    settle(&mut driver)?;
    let hand = driver.state().zone(Zone::Hand(pid(0))).len();
    assert_eq!(hand, 2);
    replays(&driver, &setup);
    Ok(())
}
