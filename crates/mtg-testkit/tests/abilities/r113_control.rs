//! Goal: a control change that ends in the cleanup step triggers what waits for it at 2 to 4 seats: Stolen Uniform's
//! "when you lose control of that Equipment this turn" fires as its until-end-of-turn control ends (CR 514.2, 514.3a)
//! and unattaches the Equipment, its intervening "if" (CR 603.4) holds it back once the Equipment is no longer attached
//! to a creature its controller controls, and each log replays to the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_engine::characteristics::controller_of;
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::events::Event;
use mtg_engine::turn::Step;
use mtg_testkit::cards::{island, mountain, plains};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::copying::replays;
use crate::common::pid;

/// Seat 0 controls Grizzly Bears, an Island and a Mountain in the first main phase, with Stolen Uniform and Lightning
/// Bolt in hand; the last seat controls an unattached Bonesplitter; each library holds Plains.
fn table(seats: usize) -> Scenario {
    let mut scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .hand_named(0, ["Stolen Uniform", "Lightning Bolt"])
        .battlefield(
            0,
            [
                ready(card("Grizzly Bears").unwrap()),
                ready(island()),
                ready(mountain()),
            ],
        )
        .battlefield(seats - 1, [ready(card("Bonesplitter").unwrap())]);
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

/// Casts Stolen Uniform on Grizzly Bears and Bonesplitter and lets it resolve.
fn steal(driver: &mut Driver) -> Result<(), ScriptError> {
    driver.begin_cast("Stolen Uniform")?;
    let bears = driver.target_named("Grizzly Bears")?;
    let splitter = driver.target_named("Bonesplitter")?;
    driver.choose_targets(&[bears])?;
    driver.choose_targets(&[splitter])?;
    driver.answer(Answer::PayMana(PayChoice::Auto))?;
    settle(driver)
}

/// How many abilities triggered in the log.
fn triggered(driver: &Driver) -> usize {
    driver
        .engine()
        .events()
        .iter()
        .filter(|event| matches!(event, Event::AbilityTriggered(_)))
        .count()
}

#[rstest]
fn the_equipment_comes_off_as_its_control_ends_in_cleanup(
    #[values(2, 3, 4)] seats: usize,
    #[values(false, true)] bolt_the_bearer: bool,
) -> Result<(), ScriptError> {
    let setup = table(seats).setup().unwrap();
    let mut driver = Driver::from_scenario(table(seats))?;
    steal(&mut driver)?;
    let (bears, splitter) = (
        driver.id_of("Grizzly Bears")?,
        driver.id_of("Bonesplitter")?,
    );
    let state = driver.state();
    assert_eq!(controller_of(state, splitter), Some(pid(0)));
    let attached = |driver: &Driver| {
        driver
            .state()
            .object(splitter)
            .and_then(|o| o.permanent())
            .and_then(mtg_engine::object::PermanentState::attached_to)
            .map(|on| on.id)
    };
    assert_eq!(attached(&driver), Some(bears));
    if bolt_the_bearer {
        driver.begin_cast("Lightning Bolt")?;
        let target = driver.target_named("Grizzly Bears")?;
        driver.choose_targets(&[target])?;
        driver.answer(Answer::PayMana(PayChoice::Auto))?;
        settle(&mut driver)?;
    }
    let before = triggered(&driver);
    driver.idle_until(2, Step::Upkeep)?;
    // Control went back with the end of the turn, and the Equipment is off the creature.
    assert_eq!(
        controller_of(driver.state(), splitter),
        Some(pid(seats - 1))
    );
    assert_eq!(attached(&driver), None);
    // The lost control triggered only while the Equipment was on seat 0's creature.
    assert_eq!(triggered(&driver) - before, usize::from(!bolt_the_bearer));
    replays(&driver, &setup);
    Ok(())
}

/// Seat 1 is active in its first main phase with Lightning Bolt and a Mountain; seat 0 holds Chef's Kiss with three
/// Mountains and controls Grizzly Bears, and with `bears` every other seat controls Grizzly Bears too.
fn kiss_table(seats: usize, bears: bool) -> Scenario {
    let mut scenario = Scenario::new(seats)
        .at(1, 1, Step::Main1)
        .hand_named(1, ["Lightning Bolt"])
        .hand_named(0, ["Chef's Kiss"]);
    let mut mine = vec![ready(card("Grizzly Bears").unwrap())];
    mine.extend((0..3).map(|_| ready(mountain())));
    scenario = scenario.battlefield(0, mine);
    for seat in 0..seats {
        let mut board = Vec::new();
        if seat == 1 {
            board.push(ready(mountain()));
        }
        if bears && seat != 0 {
            board.push(ready(card("Grizzly Bears").unwrap()));
        }
        if !board.is_empty() {
            scenario = scenario.battlefield(seat, board);
        }
        scenario = scenario.library(seat, (0..8).map(|_| plains()));
    }
    scenario
}

/// Life lost by every seat but seat 0, and 3 for each of their creatures in a graveyard.
fn harm_to_others(driver: &Driver, seats: usize, setup_life: i32) -> i32 {
    let state = driver.state();
    let mut harm = 0;
    for seat in 1..seats {
        harm += setup_life - state.player(pid(seat)).unwrap().life();
    }
    let graved = (1..seats)
        .flat_map(|seat| {
            state
                .zone(mtg_engine::zones::Zone::Graveyard(pid(seat)))
                .iter()
        })
        .filter(|id| state.object(**id).unwrap().card().name == "Grizzly Bears")
        .count();
    // A Grizzly Bears that died took a Bolt's 3 damage.
    harm + 3 * i32::try_from(graved).unwrap()
}

#[rstest]
fn a_kissed_bolt_and_its_copy_hit_only_the_others(
    #[values(2, 3, 4)] seats: usize,
    #[values(false, true)] bears: bool,
) -> Result<(), ScriptError> {
    let setup = kiss_table(seats, bears).setup().unwrap();
    let mut driver = Driver::from_scenario(kiss_table(seats, bears))?;
    let life = driver.state().player(pid(1)).unwrap().life();
    driver.begin_cast("Lightning Bolt")?;
    let mine = driver.target_named("Grizzly Bears")?;
    let mine = match mine {
        mtg_engine::choose::Target::Object(_) => {
            let id = driver
                .state()
                .zone(mtg_engine::zones::Zone::Battlefield)
                .iter()
                .copied()
                .find(|id| {
                    controller_of(driver.state(), *id) == Some(pid(0))
                        && driver.state().object(*id).unwrap().card().name == "Grizzly Bears"
                })
                .unwrap();
            mtg_engine::choose::Target::Object(driver.state().object(id).unwrap().reference())
        }
        other @ mtg_engine::choose::Target::Player(_) => other,
    };
    driver.choose_targets(&[mine])?;
    driver.answer(Answer::PayMana(PayChoice::Auto))?;
    while driver.decision()?.player != pid(0) {
        driver.pass()?;
    }
    let bolt = *driver.state().stack().last().unwrap();
    driver.begin_cast("Chef's Kiss")?;
    let spell =
        mtg_engine::choose::Target::Object(driver.state().object(bolt).unwrap().reference());
    driver.choose_targets(&[spell])?;
    driver.answer(Answer::PayMana(PayChoice::Auto))?;
    settle(&mut driver)?;
    let state = driver.state();
    // Seat 0 took nothing and kept its creature; the Bolt and its copy dealt their 6 damage among the others.
    assert_eq!(state.player(pid(0)).unwrap().life(), life);
    assert!(
        state
            .zone(mtg_engine::zones::Zone::Graveyard(pid(0)))
            .iter()
            .all(|id| { state.object(*id).unwrap().card().name != "Grizzly Bears" })
    );
    let copies: Vec<_> = driver
        .engine()
        .events()
        .iter()
        .filter_map(|event| match event {
            Event::SpellCopied(copy) => Some(copy.controller),
            _ => None,
        })
        .collect();
    assert_eq!(copies, vec![pid(0)]);
    let marked: i32 = state
        .zone(mtg_engine::zones::Zone::Battlefield)
        .iter()
        .filter_map(|id| state.object(*id).and_then(|o| o.permanent()))
        .map(|p| i32::try_from(p.damage()).unwrap())
        .sum();
    assert_eq!(harm_to_others(&driver, seats, life) + marked, 6);
    replays(&driver, &setup);
    Ok(())
}

/// Seat 0 controls Old Man of the Sea and a Forest in the first main phase with Giant Growth in hand; the last seat
/// controls Grizzly Bears.
fn sea_table(seats: usize) -> Scenario {
    let mut scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .hand_named(0, ["Giant Growth"])
        .battlefield(
            0,
            [
                ready(card("Old Man of the Sea").unwrap()),
                ready(mtg_testkit::cards::forest()),
            ],
        )
        .battlefield(seats - 1, [ready(card("Grizzly Bears").unwrap())]);
    for seat in 0..seats {
        scenario = scenario.library(seat, (0..8).map(|_| plains()));
    }
    scenario
}

#[rstest]
fn old_man_keeps_the_creature_while_its_power_stays_at_most_his(
    #[values(2, 3, 4)] seats: usize,
    #[values(false, true)] grow: bool,
) -> Result<(), ScriptError> {
    let setup = sea_table(seats).setup().unwrap();
    let mut driver = Driver::from_scenario(sea_table(seats))?;
    driver.begin_activation("Old Man of the Sea")?;
    let bears = driver.target_named("Grizzly Bears")?;
    driver.choose_targets(&[bears])?;
    settle(&mut driver)?;
    let id = driver.id_of("Grizzly Bears")?;
    assert_eq!(controller_of(driver.state(), id), Some(pid(0)));
    if grow {
        // Giant Growth makes the creature's power 5, more than Old Man's 2: the control ends at once (CR 611.2b).
        driver.begin_cast("Giant Growth")?;
        let bears = driver.target_named("Grizzly Bears")?;
        driver.choose_targets(&[bears])?;
        driver.answer(Answer::PayMana(PayChoice::Auto))?;
        settle(&mut driver)?;
        assert_eq!(controller_of(driver.state(), id), Some(pid(seats - 1)));
    }
    // The turn passes with Old Man still tapped: without the growth the creature stays seat 0's, and an ended control
    // does not come back when the growth wears off.
    driver.idle_until(2, Step::Upkeep)?;
    let expected = if grow { pid(seats - 1) } else { pid(0) };
    assert_eq!(controller_of(driver.state(), id), Some(expected));
    replays(&driver, &setup);
    Ok(())
}
