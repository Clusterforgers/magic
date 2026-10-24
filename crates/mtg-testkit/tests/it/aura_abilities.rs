//! Goal: the Auras that give a creature a quoted ability play as their cards say at tables of two, three and four seats: Level Up puts a counter on the creature as it enters and doubles the counters whenever the creature attacks (drawing a card for power 10 or more), Idolized pumps a creature that attacks alone by the nonland permanents its controller has and a creature that attacks with another is not pumped, Elemental Mastery lets the creature tap for as many hasty Elementals as its power and exiles them at the next end step; the ability is the creature's and its controller's, and the log replays to the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_cards::value::CounterKind;
use mtg_engine::characteristics;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_engine::{GameSetup, replay};
use mtg_testkit::cards::{forest, free_creature, mountain};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, attached_to, ready, with_counters};
use rstest::rstest;

use crate::common::casting::lands;
use crate::common::{names, pid};

/// Passes priority until the stack is empty (at most a few rounds).
fn resolve_all(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..24 {
        if driver.state().stack().is_empty() && driver.state().resolving().is_none() {
            return Ok(());
        }
        driver.pass()?;
    }
    Err(ScriptError::Stuck("the stack".into()))
}

fn replays(driver: &Driver, setup: &GameSetup) {
    let replayed = replay(setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
}

fn counters(driver: &Driver, name: &str) -> u32 {
    let id = driver.id_of(name).unwrap();
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

fn power(driver: &Driver, name: &str) -> i32 {
    let id = driver.id_of(name).unwrap();
    characteristics(driver.state(), id).unwrap().power.unwrap()
}

/// Seat 0 has Mine (a 2/2 with `held` +1/+1 counters) wearing `aura`, then `others`; every seat has a library.
fn worn(
    seats: usize,
    aura: &str,
    held: u32,
    others: Vec<mtg_engine::setup::PresetPermanent>,
) -> Scenario {
    let mine = with_counters(
        ready(free_creature("Mine", 2, 2)),
        CounterKind::PlusOnePlusOne,
        held,
    );
    let mut field = vec![mine, attached_to(ready(card(aura).unwrap()), 0, 0)];
    field.extend(others);
    let mut scenario = Scenario::new(seats)
        .at(2, 0, Step::DeclareAttackers)
        .battlefield(0, field);
    for seat in 0..seats {
        scenario = scenario.library(seat, (0..12).map(|_| free_creature("Filler", 1, 1)));
    }
    scenario
}

#[rstest]
#[case::no_counters_to_double(0, 0, 0)]
#[case::one_counter(1, 2, 0)]
#[case::three_counters(3, 6, 0)]
#[case::power_ten_draws(4, 8, 1)]
fn level_up_doubles_the_counters_when_the_creature_attacks(
    #[values(2, 3, 4)] seats: usize,
    #[case] held: u32,
    #[case] after: u32,
    #[case] drawn: usize,
) -> Result<(), ScriptError> {
    let game = || worn(seats, "Level Up", held, vec![]);
    let setup = game().setup().unwrap();
    let mut driver = Driver::from_scenario(game())?;
    let hand = names(driver.state(), Zone::Hand(pid(0))).len();
    driver.attack(&[("Mine", 1)])?;
    resolve_all(&mut driver)?;
    assert_eq!(counters(&driver, "Mine"), after);
    assert_eq!(
        names(driver.state(), Zone::Hand(pid(0))).len(),
        hand + drawn
    );
    replays(&driver, &setup);
    Ok(())
}

#[rstest]
fn level_up_puts_a_counter_on_the_creature_as_it_enters(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut field = lands(2, forest);
    field.push(ready(free_creature("Mine", 2, 2)));
    let mut scenario = Scenario::new(seats)
        .hand_named(0, ["Level Up"])
        .battlefield(0, field);
    for seat in 0..seats {
        scenario = scenario.library(seat, (0..8).map(|_| free_creature("Filler", 1, 1)));
    }
    let mut driver = Driver::from_scenario(scenario)?;
    let mine = driver.target_named("Mine")?;
    driver.cast_at("Level Up", &[mine])?;
    resolve_all(&mut driver)?;
    assert_eq!(counters(&driver, "Mine"), 1);
    assert_eq!(power(&driver, "Mine"), 3);
    Ok(())
}

/// Idolized: the creature gets +X/+X for the nonland permanents its controller has (the Aura and the creature among them).
#[rstest]
#[case::alone_with_nothing_else(&[], &["Mine"], 4)]
#[case::four_permanents_of_ours(&["Ally", "Ornament"], &["Mine"], 6)]
#[case::attacking_with_another_is_no_trigger(&["Ally"], &["Mine", "Ally"], 2)]
fn idolized_pumps_a_creature_that_attacks_alone(
    #[values(2, 3, 4)] seats: usize,
    #[case] others: &[&str],
    #[case] attackers: &[&str],
    #[case] expected: i32,
) -> Result<(), ScriptError> {
    let extra = || {
        let mut list = lands(2, mountain);
        list.extend(others.iter().map(|name| ready(free_creature(name, 1, 1))));
        list
    };
    let game = || worn(seats, "Idolized", 0, extra());
    let setup = game().setup().unwrap();
    let mut driver = Driver::from_scenario(game())?;
    let attacks: Vec<(&str, usize)> = attackers.iter().map(|name| (*name, 1)).collect();
    driver.attack(&attacks)?;
    resolve_all(&mut driver)?;
    assert_eq!(power(&driver, "Mine"), expected);
    replays(&driver, &setup);
    Ok(())
}

/// Elemental Mastery: tapping the creature makes as many 1/1 hasty Elementals as its power, exiled at the next end step.
#[rstest]
#[case::power_two(2, 0, 2)]
#[case::power_three_with_a_counter(2, 1, 3)]
#[case::power_zero_makes_none(0, 0, 0)]
fn elemental_mastery_makes_elementals_equal_to_the_power_and_exiles_them(
    #[values(2, 3, 4)] seats: usize,
    #[case] printed: i32,
    #[case] held: u32,
    #[case] made: usize,
) -> Result<(), ScriptError> {
    let game = || {
        let mine = with_counters(
            ready(free_creature("Mine", printed, 2)),
            CounterKind::PlusOnePlusOne,
            held,
        );
        let mut scenario = Scenario::new(seats).battlefield(
            0,
            [
                mine,
                attached_to(ready(card("Elemental Mastery").unwrap()), 0, 0),
            ],
        );
        for seat in 0..seats {
            scenario = scenario.library(seat, (0..12).map(|_| free_creature("Filler", 1, 1)));
        }
        scenario
    };
    let setup = game().setup().unwrap();
    let mut driver = Driver::from_scenario(game())?;
    driver.begin_activation("Mine")?;
    resolve_all(&mut driver)?;
    let elementals = |driver: &Driver| {
        names(driver.state(), Zone::Battlefield)
            .iter()
            .filter(|name| *name == "Elemental")
            .count()
    };
    assert_eq!(elementals(&driver), made);
    let turn = driver.state().turn().number();
    driver.idle_until(turn, Step::Main2)?;
    assert_eq!(elementals(&driver), made);
    driver.idle_until(turn + 1, Step::Upkeep)?;
    assert_eq!(elementals(&driver), 0);
    replays(&driver, &setup);
    Ok(())
}
