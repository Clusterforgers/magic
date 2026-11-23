//! Goal: "Choose one. If this spell was kicked, choose any number instead" (Inscription of Abundance) asks the kicker with the modes and before them (CR 601.2b), offers up to every mode when kicked and one when not, at tables of two and four seats: the kicked modes chosen all resolve in printed order, a choice of two is refused unkicked, and the log replays to the state it was played to.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;
use common::{names, pid};
use mtg_engine::choose::{CostsChoice, ModeChoice, ModeOptions};
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::error::{EngineError, IllegalAnswer};
use mtg_engine::replay;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{forest, free_creature};
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

const SPELL: &str = "Inscription of Abundance";

/// Seat 0 holds the spell, six Forests and a 3/3 Ox; seat 1 controls a 2/2 Bear.
fn table(seats: usize) -> Result<Driver, ScriptError> {
    let mut scenario = Scenario::new(seats)
        .hand_named(0, [SPELL])
        .battlefield(
            0,
            (0..6)
                .map(|_| ready(forest()))
                .chain([ready(free_creature("Ox", 3, 3))]),
        )
        .battlefield(1, [ready(free_creature("Bear", 2, 2))]);
    for seat in 0..seats {
        scenario = scenario.library(seat, (0..8).map(|_| forest()));
    }
    Driver::from_scenario(scenario)
}

/// Begins the cast and answers the kicker, which is asked before the modes; returns the mode options.
fn announce(driver: &mut Driver, kicked: bool) -> Result<ModeOptions, ScriptError> {
    driver.begin_cast(SPELL)?;
    driver.expect_decision(0, "choose-costs")?;
    let costs = if kicked { vec![0] } else { Vec::new() };
    driver.answer(Answer::ChooseCosts(CostsChoice::Costs(costs)))?;
    match &driver.decision()?.kind {
        DecisionKind::ChooseMode(options) => Ok(options.clone()),
        other => panic!("the modes follow the kicker, got {}", other.name()),
    }
}

fn counters(driver: &Driver, name: &str) -> u32 {
    let id = driver.id_of(name).unwrap();
    let object = driver.state().object(id).unwrap();
    object.permanent().unwrap().counters().map(|(_, n)| n).sum()
}

#[rstest]
#[case::unkicked(false, 1)]
#[case::kicked(true, 3)]
fn the_most_modes_follow_the_kicker(
    #[values(2, 4)] seats: usize,
    #[case] kicked: bool,
    #[case] most: u32,
) -> Result<(), ScriptError> {
    let mut driver = table(seats)?;
    let options = announce(&mut driver, kicked)?;
    assert_eq!((options.min, options.max), (1, most));
    assert_eq!(options.modes.len(), 3);
    Ok(())
}

#[rstest]
fn two_modes_are_refused_unkicked_and_change_nothing(
    #[values(2, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = table(seats)?;
    announce(&mut driver, false)?;
    let before = (driver.engine().state_hash(), driver.engine().events().len());
    let error = driver
        .answer(Answer::ChooseMode(ModeChoice::Modes(vec![0, 2])))
        .unwrap_err();
    assert_eq!(
        error,
        ScriptError::Engine(EngineError::Illegal(IllegalAnswer::WrongCount {
            expected: 1,
            got: 2
        }))
    );
    assert_eq!(
        (driver.engine().state_hash(), driver.engine().events().len()),
        before
    );
    Ok(())
}

#[rstest]
#[case::counters_and_fight(vec![0, 2], 20)]
#[case::all_three(vec![0, 1, 2], 25)]
fn the_kicked_modes_all_resolve(
    #[values(2, 4)] seats: usize,
    #[case] modes: Vec<usize>,
    #[case] life: i32,
) -> Result<(), ScriptError> {
    let mut driver = table(seats)?;
    announce(&mut driver, true)?;
    driver.answer(Answer::ChooseMode(ModeChoice::Modes(modes.clone())))?;
    let ox = driver.target_named("Ox")?;
    let bear = driver.target_named("Bear")?;
    driver.choose_targets(&[ox])?;
    if modes.contains(&1) {
        let me = driver.target_player(0);
        driver.choose_targets(&[me])?;
    }
    driver.choose_targets(&[ox])?;
    driver.choose_targets(&[bear])?;
    driver.answer(Answer::PayMana(PayChoice::Auto))?;
    while !driver.state().stack().is_empty() {
        driver.pass()?;
    }
    assert_eq!(counters(&driver, "Ox"), 2, "two +1/+1 counters on the Ox");
    assert!(
        names(driver.state(), Zone::Graveyard(pid(1))).contains(&"Bear".to_owned()),
        "the 5/5 Ox fights and destroys the Bear"
    );
    // Life is gained after the counters, so the greatest power is the Ox's 5.
    assert_eq!(driver.state().player(pid(0)).unwrap().life(), life);
    let copy = replay(driver.engine().setup(), driver.engine().events())?;
    assert_eq!(copy.state_hash(), driver.engine().state_hash());
    Ok(())
}
