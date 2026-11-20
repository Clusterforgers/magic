//! Goal: abilities read the choices of the spell that triggers them at 2 to 4 seats: Riku of Many Paths offers up to as
//! many modes as were chosen for the modal spell cast (CR 700.2) and never triggers on a spell without modes; Unbound
//! Flourishing doubles the X of a permanent spell with {X} in its mana cost (CR 107.3) and copies an instant with {X}
//! in its mana cost and an ability with {X} in its activation cost, not one without; a target "of an opponent's choice" is
//! chosen by the opponent the caster picks as the spell is cast (Volcanic Offering); each log replays to the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_cards::value::CounterKind;
use mtg_engine::choose::{ModeChoice, NumberChoice, OptionChoice};
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{mountain, plains, swamp};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::copying::replays;
use crate::common::{names, pid};

/// Seat 0 controls `permanents` and four ready Plains, Swamps and Mountains in the first main phase, with `hand` in hand.
fn table(seats: usize, permanents: &[&str], hand: &[&str]) -> Scenario {
    let mut board: Vec<_> = permanents
        .iter()
        .map(|name| ready(card(name).unwrap()))
        .collect();
    board.extend((0..4).map(|_| ready(plains())));
    board.extend((0..4).map(|_| ready(swamp())));
    board.extend((0..4).map(|_| ready(mountain())));
    let mut scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .hand_named(0, hand.iter().copied())
        .battlefield(0, board)
        .library(0, (0..8).map(|_| plains()));
    for seat in 1..seats {
        scenario = scenario.library(seat, (0..8).map(|_| plains()));
    }
    scenario
}

/// Plays on until the stack is empty, keeping the targets of every copy; Riku's modes are `riku`, after the most it
/// offered is noted.
fn settle(driver: &mut Driver, riku: &[usize]) -> Result<Option<u32>, ScriptError> {
    let mut offered = None;
    for _ in 0..200 {
        if driver.state().stack().is_empty() && driver.state().resolving().is_none() {
            return Ok(offered);
        }
        match &driver.decision()?.kind {
            DecisionKind::ChooseMode(options) => {
                offered = Some(options.max);
                driver.answer(Answer::ChooseMode(ModeChoice::Modes(riku.to_vec())))?;
            }
            DecisionKind::PayMana(_) => {
                driver.answer(Answer::PayMana(PayChoice::Auto))?;
            }
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    Err(ScriptError::Stuck("the stack".into()))
}

/// The +1/+1 counters on the permanent of the name.
fn counters(driver: &Driver, name: &str) -> u32 {
    let id = driver.id_of(name).unwrap();
    driver
        .state()
        .object(id)
        .unwrap()
        .permanent()
        .unwrap()
        .counters_of(&CounterKind::PlusOnePlusOne)
}

fn life(driver: &Driver, seat: usize) -> i32 {
    driver.state().player(pid(seat)).unwrap().life()
}

#[rstest]
fn riku_offers_as_many_modes_as_the_spell_chose(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = || table(seats, &["Riku of Many Paths"], &["Austere Command"]);
    let setup = scenario().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario())?;
    driver.begin_cast("Austere Command")?;
    driver.answer(Answer::ChooseMode(ModeChoice::Modes(vec![0, 1])))?;
    driver.answer(Answer::PayMana(PayChoice::Auto))?;
    // Two modes were chosen, so Riku offers two of its three; it takes the counter and the Bird.
    assert_eq!(settle(&mut driver, &[1, 2])?, Some(2));
    assert_eq!(counters(&driver, "Riku of Many Paths"), 1);
    assert!(driver.id_of("Bird").is_ok());
    replays(&driver, &setup);
    Ok(())
}

#[rstest]
fn riku_ignores_a_spell_without_modes(#[values(2, 3, 4)] seats: usize) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table(seats, &["Riku of Many Paths"], &["Blaze"]))?;
    driver.begin_cast("Blaze")?;
    driver.answer(Answer::ChooseNumber(NumberChoice::Number(1)))?;
    let foe = driver.target_player(1);
    driver.choose_targets(&[foe])?;
    driver.answer(Answer::PayMana(PayChoice::Auto))?;
    assert_eq!(settle(&mut driver, &[])?, None);
    assert_eq!(life(&driver, 1), 19);
    Ok(())
}

#[rstest]
fn flourishing_doubles_the_x_of_a_permanent_spell(
    #[values(2, 3, 4)] seats: usize,
    #[values(1, 3)] x: u32,
) -> Result<(), ScriptError> {
    let scenario = || table(seats, &["Unbound Flourishing"], &["Endless One"]);
    let setup = scenario().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario())?;
    driver.begin_cast("Endless One")?;
    driver.answer(Answer::ChooseNumber(NumberChoice::Number(x)))?;
    driver.answer(Answer::PayMana(PayChoice::Auto))?;
    settle(&mut driver, &[])?;
    assert_eq!(counters(&driver, "Endless One"), 2 * x);
    replays(&driver, &setup);
    Ok(())
}

#[rstest]
fn flourishing_copies_an_instant_with_x(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = || table(seats, &["Unbound Flourishing"], &["Blaze"]);
    let setup = scenario().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario())?;
    driver.begin_cast("Blaze")?;
    driver.answer(Answer::ChooseNumber(NumberChoice::Number(2)))?;
    let foe = driver.target_player(1);
    driver.choose_targets(&[foe])?;
    driver.answer(Answer::PayMana(PayChoice::Auto))?;
    settle(&mut driver, &[])?;
    // The copy has the same X and target: 2 and 2.
    assert_eq!(life(&driver, 1), 16);
    replays(&driver, &setup);
    Ok(())
}

#[rstest]
fn flourishing_copies_an_ability_with_x_in_its_cost(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = || table(seats, &["Unbound Flourishing", "Crypt Rats"], &[]);
    let setup = scenario().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario())?;
    driver.begin_activation("Crypt Rats")?;
    driver.answer(Answer::ChooseNumber(NumberChoice::Number(1)))?;
    driver.answer(Answer::PayMana(PayChoice::Auto))?;
    settle(&mut driver, &[])?;
    for seat in 0..seats {
        assert_eq!(life(&driver, seat), 18, "seat {seat}");
    }
    replays(&driver, &setup);
    Ok(())
}

#[rstest]
fn flourishing_leaves_an_instant_without_x(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver =
        Driver::from_scenario(table(seats, &["Unbound Flourishing"], &["Lightning Bolt"]))?;
    driver.begin_cast("Lightning Bolt")?;
    let foe = driver.target_player(1);
    driver.choose_targets(&[foe])?;
    driver.answer(Answer::PayMana(PayChoice::Auto))?;
    settle(&mut driver, &[])?;
    assert_eq!(life(&driver, 1), 17);
    Ok(())
}

/// Seat 0 holds Volcanic Offering with Mountains ready; every other seat controls a Reliquary Tower and a Grizzly
/// Bears.
fn offering_table(seats: usize) -> Scenario {
    let mut scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .hand_named(0, ["Volcanic Offering"])
        .battlefield(0, (0..5).map(|_| ready(mountain())))
        .library(0, (0..8).map(|_| plains()));
    for seat in 1..seats {
        scenario = scenario
            .battlefield_named(seat, ["Reliquary Tower", "Grizzly Bears"])
            .library(seat, (0..8).map(|_| plains()));
    }
    scenario
}

#[rstest]
fn an_opponent_the_caster_picks_chooses_the_targets_of_their_choice(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let setup = offering_table(seats).setup().unwrap();
    let mut driver = Driver::from_scenario(offering_table(seats))?;
    driver.begin_cast("Volcanic Offering")?;
    // The caster picks the last opponent offered; each picker takes the last legal target.
    let mut pickers = Vec::new();
    let mut offered = Vec::new();
    loop {
        let decision = driver.decision()?.clone();
        match &decision.kind {
            DecisionKind::ChooseOption(options) => {
                assert_eq!(decision.player, pid(0));
                offered.push(options.options.len());
                let last = options.options.len() - 1;
                driver.answer(Answer::ChooseOption(OptionChoice(last)))?;
            }
            DecisionKind::ChooseTargets(options) => {
                pickers.push(decision.player);
                let last = *options.legal.last().unwrap();
                driver.choose_targets(&[last])?;
            }
            _ => break,
        }
    }
    let last = pid(seats - 1);
    assert_eq!(pickers, [pid(0), last, pid(0), last]);
    // Only a table with more than one opponent asks which opponent chooses.
    let asked = if seats > 2 {
        vec![seats - 1, seats - 1]
    } else {
        Vec::new()
    };
    assert_eq!(offered, asked);
    driver.answer(Answer::PayMana(PayChoice::Auto))?;
    settle(&mut driver, &[])?;
    // Both picked the last opponent's land and creature: they are gone.
    let graveyard = names(driver.state(), Zone::Graveyard(last));
    assert!(
        graveyard.contains(&"Reliquary Tower".to_owned()),
        "{graveyard:?}"
    );
    assert!(
        graveyard.contains(&"Grizzly Bears".to_owned()),
        "{graveyard:?}"
    );
    replays(&driver, &setup);
    Ok(())
}
