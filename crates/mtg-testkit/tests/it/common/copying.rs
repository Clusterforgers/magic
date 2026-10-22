//! Goal: helpers for the tests of copies of spells and abilities and of storm (this is not a test module itself).
#![allow(dead_code, clippy::unwrap_used, clippy::expect_used)]

use mtg_core::ids::ObjectId;
use mtg_engine::choose::{Target, TargetChoice, YesNoChoice};
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::zones::Zone;
use mtg_engine::{Event, GameSetup, PresetPermanent, replay};
use mtg_testkit::cards::{island, mountain};
use mtg_testkit::{Driver, Scenario, ScriptError, ready};

use super::pid;

/// `islands` untapped Islands then `mountains` Mountains.
pub fn lands(islands: usize, mountains: usize) -> Vec<PresetPermanent> {
    (0..islands)
        .map(|_| ready(island()))
        .chain((0..mountains).map(|_| ready(mountain())))
        .collect()
}

/// Seat 0 holds the corpus cards `hand` and has `islands` Islands and `mountains` Mountains.
pub fn game(seats: usize, hand: &[&str], islands: usize, mountains: usize) -> Scenario {
    Scenario::new(seats)
        .hand_named(0, hand.iter().copied())
        .battlefield(0, lands(islands, mountains))
}

/// Whether a priority decision is pending.
pub fn has_priority(driver: &Driver) -> bool {
    matches!(
        driver.engine().decision().map(|d| &d.kind),
        Some(DecisionKind::Priority(_))
    )
}

/// Every seat passes once: the top of the stack resolves, or a question is asked.
pub fn pass_round(driver: &mut Driver, seats: usize) -> Result<(), ScriptError> {
    for _ in 0..seats {
        if !has_priority(driver) {
            break;
        }
        driver.pass()?;
    }
    Ok(())
}

/// Passes priority round by round until a question other than priority is asked or the stack is empty.
pub fn run(driver: &mut Driver, seats: usize) -> Result<(), ScriptError> {
    for _ in 0..400 {
        if driver.state().stack().is_empty() && driver.state().resolving().is_none() {
            return Ok(());
        }
        if !has_priority(driver) {
            return Ok(());
        }
        pass_round(driver, seats)?;
    }
    Err(ScriptError::Stuck("the stack".into()))
}

/// Answers every question passively and passes until the stack is empty.
pub fn settle(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..400 {
        if driver.state().stack().is_empty() && driver.state().resolving().is_none() {
            return Ok(());
        }
        driver.answer_passively()?;
    }
    Err(ScriptError::Stuck("the stack".into()))
}

pub fn life(driver: &Driver, seat: usize) -> i32 {
    driver.state().player(pid(seat)).unwrap().life()
}

pub fn hand(driver: &Driver, seat: usize) -> usize {
    driver.state().zone(Zone::Hand(pid(seat))).len()
}

/// The copies of spells on the stack, bottom first.
pub fn spell_copies(driver: &Driver) -> Vec<ObjectId> {
    driver
        .state()
        .stack()
        .iter()
        .copied()
        .filter(|id| driver.state().object(*id).unwrap().is_spell_copy())
        .collect()
}

/// The copies of abilities on the stack, bottom first.
pub fn ability_copies(driver: &Driver) -> Vec<ObjectId> {
    driver
        .state()
        .stack()
        .iter()
        .copied()
        .filter(|id| {
            let object = driver.state().object(*id).unwrap();
            object.ability().is_some() && object.copied_from().is_some()
        })
        .collect()
}

/// The object of the card `name` on the stack that is no copy, as a target.
pub fn on_the_stack(driver: &Driver, name: &str) -> Target {
    let id = driver
        .state()
        .stack()
        .iter()
        .copied()
        .find(|id| {
            let object = driver.state().object(*id).unwrap();
            object.card().name == name && object.copied_from().is_none()
        })
        .unwrap_or_else(|| panic!("{name} is on the stack"));
    Target::Object(driver.state().object(id).unwrap().reference())
}

/// The object a target names.
pub fn object_of(target: Target) -> ObjectId {
    target.object().unwrap().id
}

pub fn say(driver: &mut Driver, yes: bool) -> Result<Vec<Event>, ScriptError> {
    driver.answer(Answer::YesNo(YesNoChoice(yes)))
}

/// The log of the driver replays to the live state.
pub fn replays(driver: &Driver, setup: &GameSetup) {
    let replayed = replay(setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
}

/// How many spells were cast so far.
pub fn casts(driver: &Driver) -> usize {
    driver
        .engine()
        .events()
        .iter()
        .filter(|e| matches!(e, Event::SpellCast { .. }))
        .count()
}

/// Casts Twincast at the spell `name` on the stack, tapping two Islands.
pub fn twincast(driver: &mut Driver, name: &str) -> Result<(), ScriptError> {
    let spell = on_the_stack(driver, name);
    driver.begin_cast("Twincast")?;
    driver.choose_targets(&[spell])?;
    driver.pay_tapping(&["Island", "Island"], 0)?;
    Ok(())
}

/// Answers No to every question about new targets and passes until the stack is empty.
pub fn keep_targets_until_done(driver: &mut Driver, seats: usize) -> Result<(), ScriptError> {
    for _ in 0..200 {
        if driver.state().stack().is_empty() && driver.state().resolving().is_none() {
            return Ok(());
        }
        match &driver.decision()?.kind {
            DecisionKind::YesNo(_) => {
                say(driver, false)?;
            }
            DecisionKind::ChooseTargets(options) => {
                let chosen = options
                    .legal
                    .iter()
                    .copied()
                    .take(options.min as usize)
                    .collect();
                driver.answer(Answer::ChooseTargets(TargetChoice::Targets(chosen)))?;
            }
            _ => pass_round(driver, seats)?,
        }
    }
    Err(ScriptError::Stuck("the stack".into()))
}
