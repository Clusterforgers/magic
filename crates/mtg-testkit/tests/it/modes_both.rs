//! Goal: "If you control a commander as you cast this spell, you may choose both instead" (Jeska's Will, Akroma's Will) offers one mode, or one or both while a commander is on the battlefield, at tables of two, three and four seats: the condition is read as the spell is cast, both modes happen in printed order when both are chosen, a choice of both is refused without the commander, and the log replays to the state it was played to.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;
use common::names;
use mtg_core::color::Color;
use mtg_engine::choose::{ModeChoice, ModeOptions};
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::error::{EngineError, IllegalAnswer};
use mtg_engine::format::Commander;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_engine::{Event, replay};
use mtg_testkit::cards::{mountain, plains};
use mtg_testkit::commander_cards::legend;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;
use std::sync::Arc;

use common::pid;

/// Seat 0 has the spell and four Mountains in the first main phase of turn 1; the others hold `opposing` cards in hand.
fn table(seats: usize, spell: &str, with_commander: bool) -> Result<Driver, ScriptError> {
    let mut scenario = Scenario::new(seats)
        .format(Arc::new(Commander))
        .commanders(0, [legend("Cmd", 1, &[Color::Red], 2, 2)])
        .hand_named(0, [spell])
        .battlefield(
            0,
            (0..6)
                .map(|_| ready(mountain()))
                .chain((0..4).map(|_| ready(plains()))),
        )
        .library(0, (0..8).map(|_| mountain()))
        .at(1, 0, Step::Main1);
    for seat in 1..seats {
        scenario = scenario
            .hand(seat, (0..3).map(|_| mountain()))
            .library(seat, (0..8).map(|_| mountain()));
    }
    let mut driver = Driver::from_scenario(scenario)?;
    if with_commander {
        driver.cast("Cmd")?;
        for _ in 0..seats {
            driver.pass()?;
        }
    }
    Ok(driver)
}

fn modes(driver: &Driver) -> ModeOptions {
    match &driver.decision().unwrap().kind {
        DecisionKind::ChooseMode(options) => options.clone(),
        other => panic!("a choice of modes was expected, got {}", other.name()),
    }
}

#[rstest]
#[case::without_a_commander(false, 1)]
#[case::with_the_commander_in_play(true, 2)]
fn the_most_modes_follow_the_commander_on_the_battlefield(
    #[values(2, 3, 4)] seats: usize,
    #[values("Jeska's Will", "Akroma's Will")] spell: &str,
    #[case] with_commander: bool,
    #[case] most: u32,
) -> Result<(), ScriptError> {
    let mut driver = table(seats, spell, with_commander)?;
    driver.begin_cast(spell)?;
    let options = modes(&driver);
    assert_eq!((options.min, options.max, options.repeat), (1, most, false));
    assert_eq!(options.modes.len(), 2);
    Ok(())
}

#[rstest]
fn both_modes_are_refused_without_a_commander_and_change_nothing(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = table(seats, "Akroma's Will", false)?;
    driver.begin_cast("Akroma's Will")?;
    let before = (driver.engine().state_hash(), driver.engine().events().len());
    let error = driver
        .answer(Answer::ChooseMode(ModeChoice::Modes(vec![0, 1])))
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
fn both_modes_of_jeskas_will_add_mana_for_the_hand_and_exile_three_cards(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = table(seats, "Jeska's Will", true)?;
    driver.begin_cast("Jeska's Will")?;
    driver.answer(Answer::ChooseMode(ModeChoice::Modes(vec![0, 1])))?;
    let target = driver.target_player(1);
    driver.choose_targets(&[target])?;
    let begun = driver.engine().events().len();
    driver.answer(Answer::PayMana(PayChoice::Auto))?;
    while !driver.state().stack().is_empty() {
        driver.pass()?;
    }
    let played = &driver.engine().events()[begun..];
    let resolved = played
        .iter()
        .position(|event| matches!(event, Event::ResolutionBegan { .. }))
        .unwrap();
    let red = played[resolved..]
        .iter()
        .filter(|event| matches!(event, Event::ManaAdded { player, .. } if *player == pid(0)))
        .count();
    assert_eq!(red, 3, "one red mana for each of the three cards in hand");
    assert_eq!(
        names(driver.state(), Zone::Exile).len(),
        3,
        "three cards exiled"
    );
    assert_eq!(names(driver.state(), Zone::Library(pid(0))).len(), 5);
    let copy = replay(driver.engine().setup(), driver.engine().events())?;
    assert_eq!(copy.state_hash(), driver.engine().state_hash());
    Ok(())
}
