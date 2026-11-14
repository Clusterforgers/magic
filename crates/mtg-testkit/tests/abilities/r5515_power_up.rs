//! Goal: power-up abilities play by their keyword at tables of two to four seats: each is activated once for its
//! permanent and no more, its cost is reduced by the permanent's mana cost while it entered this turn (Jack of Hearts
//! cast this turn powers up for {3} instead of {4}{R}{R}), Wonder Man lets each power-up ability of its controller's
//! permanents be activated an additional time, Kang's power-up takes an extra turn during which no power-up ability can
//! be activated, and Marvel Boy gets a counter when another creature enters under its controller and when its
//! controller activates a power-up ability.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::value::CounterKind;
use mtg_core::ids::PlayerId;
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{bear, free_creature, island, mountain, plains};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// Passes priority until the stack is empty, answering anything else passively.
fn r5515_settle(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..200 {
        if driver.state().stack().is_empty() && driver.state().resolving().is_none() {
            return Ok(());
        }
        driver.answer_passively()?;
    }
    Err(ScriptError::Stuck("the stack".into()))
}

/// The +1/+1 counters on the permanent named `name`.
fn r5515_counters(driver: &Driver, name: &str) -> u32 {
    let id = driver.id_of(name).unwrap();
    driver
        .state()
        .object(id)
        .unwrap()
        .permanent()
        .unwrap()
        .counters_of(&CounterKind::PlusOnePlusOne)
}

/// Activates the power-up ability of `name` (its only activated ability that is not a mana ability) and pays.
fn r5515_power_up(driver: &mut Driver, name: &str) -> Result<(), ScriptError> {
    driver.begin_activation(name)?;
    if matches!(driver.decision()?.kind, DecisionKind::PayMana(_)) {
        driver.answer(Answer::PayMana(PayChoice::Auto))?;
    }
    r5515_settle(driver)
}

/// Whether the deciding player may activate an ability of `name` now.
fn r5515_offered(driver: &Driver, name: &str) -> bool {
    driver
        .offered_abilities(name)
        .is_ok_and(|offered| !offered.is_empty())
}

/// Seat 0 with `permanents` and `lands` ready, `hand` and a library of Plains, every other seat with a Bear.
fn r5515_table(
    seats: usize,
    permanents: Vec<Arc<CardDefinition>>,
    lands: Vec<Arc<CardDefinition>>,
    hand: Vec<Arc<CardDefinition>>,
) -> Result<Driver, ScriptError> {
    let mut mine: Vec<_> = permanents.into_iter().map(ready).collect();
    mine.extend(lands.into_iter().map(ready));
    let mut scenario = Scenario::new(seats)
        .battlefield(0, mine)
        .hand(0, hand)
        .library(0, (0..10).map(|_| plains()).collect::<Vec<_>>());
    for seat in 1..seats {
        scenario = scenario
            .battlefield(seat, [ready(bear())])
            .library(seat, (0..10).map(|_| plains()).collect::<Vec<_>>());
    }
    Driver::from_scenario(scenario.at(1, 0, Step::Main1))
}

#[rstest]
#[case::two_seats(2)]
#[case::three_seats(3)]
#[case::four_seats(4)]
fn r5515_a_power_up_ability_is_activated_once(#[case] seats: usize) -> Result<(), ScriptError> {
    let mut driver = r5515_table(
        seats,
        vec![card("Jack of Hearts, Volatile Hero").unwrap()],
        (0..12).map(|_| mountain()).collect(),
        Vec::new(),
    )?;
    assert!(r5515_offered(&driver, "Jack of Hearts, Volatile Hero"));
    r5515_power_up(&mut driver, "Jack of Hearts, Volatile Hero")?;
    assert_eq!(r5515_counters(&driver, "Jack of Hearts, Volatile Hero"), 2);
    assert!(
        !r5515_offered(&driver, "Jack of Hearts, Volatile Hero"),
        "a power-up ability is activated only once"
    );
    Ok(())
}

#[rstest]
#[case::two_seats(2)]
#[case::four_seats(4)]
fn r5515_a_power_up_costs_less_by_the_mana_cost_the_turn_it_entered(
    #[case] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = r5515_table(
        seats,
        Vec::new(),
        (0..6).map(|_| mountain()).collect(),
        vec![card("Jack of Hearts, Volatile Hero").unwrap()],
    )?;
    driver.cast("Jack of Hearts, Volatile Hero")?;
    r5515_settle(&mut driver)?;
    // Three Mountains are left: {4}{R}{R} less {1}{R}{R} is {3}.
    r5515_power_up(&mut driver, "Jack of Hearts, Volatile Hero")?;
    assert_eq!(r5515_counters(&driver, "Jack of Hearts, Volatile Hero"), 2);
    let untapped = driver
        .state()
        .zone(Zone::Battlefield)
        .iter()
        .filter(|id| {
            let object = driver.state().object(**id).unwrap();
            object.card().name == "Mountain" && !object.permanent().unwrap().tapped()
        })
        .count();
    assert_eq!(
        untapped, 0,
        "the reduced power-up took the last three lands"
    );
    Ok(())
}

#[rstest]
#[case::two_seats(2)]
#[case::three_seats(3)]
#[case::four_seats(4)]
fn r5515_wonder_man_lets_each_power_up_be_activated_an_additional_time(
    #[case] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = r5515_table(
        seats,
        vec![
            card("Wonder Man, Hollywood Hero").unwrap(),
            card("Jack of Hearts, Volatile Hero").unwrap(),
        ],
        (0..26).map(|_| mountain()).collect(),
        Vec::new(),
    )?;
    for _ in 0..2 {
        r5515_power_up(&mut driver, "Jack of Hearts, Volatile Hero")?;
    }
    assert_eq!(r5515_counters(&driver, "Jack of Hearts, Volatile Hero"), 4);
    assert!(
        !r5515_offered(&driver, "Jack of Hearts, Volatile Hero"),
        "twice and no more"
    );
    r5515_power_up(&mut driver, "Wonder Man, Hollywood Hero")?;
    assert_eq!(r5515_counters(&driver, "Wonder Man, Hollywood Hero"), 2);
    assert!(r5515_offered(&driver, "Wonder Man, Hollywood Hero"));
    Ok(())
}

#[rstest]
#[case::two_seats(2)]
#[case::three_seats(3)]
#[case::four_seats(4)]
fn r5515_kang_takes_an_extra_turn_without_power_ups(
    #[case] seats: usize,
) -> Result<(), ScriptError> {
    let mut lands: Vec<_> = (0..8).map(|_| island()).collect();
    lands.extend((0..7).map(|_| mountain()));
    let mut driver = r5515_table(
        seats,
        vec![
            card("Kang the Conqueror").unwrap(),
            card("Wonder Man, Hollywood Hero").unwrap(),
        ],
        lands,
        Vec::new(),
    )?;
    driver.begin_activation("Kang the Conqueror")?;
    driver.answer(Answer::PayMana(PayChoice::Auto))?;
    r5515_settle(&mut driver)?;
    assert_eq!(r5515_counters(&driver, "Kang the Conqueror"), 1);
    driver.idle_until(2, Step::Main1)?;
    assert_eq!(driver.state().turn().active(), PlayerId::from_index(0));
    assert!(
        !r5515_offered(&driver, "Wonder Man, Hollywood Hero"),
        "no power-up during the extra turn"
    );
    driver.idle_until(3, Step::Main1)?;
    assert_ne!(driver.state().turn().active(), PlayerId::from_index(0));
    Ok(())
}

#[rstest]
#[case::two_seats(2)]
#[case::three_seats(3)]
fn r5515_marvel_boy_grows_on_entering_creatures_and_power_ups(
    #[case] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = r5515_table(
        seats,
        vec![
            card("Marvel Boy, Noh-Varr").unwrap(),
            card("Jack of Hearts, Volatile Hero").unwrap(),
        ],
        (0..10).map(|_| mountain()).collect(),
        vec![free_creature("Pip", 1, 1)],
    )?;
    driver.cast("Pip")?;
    r5515_settle(&mut driver)?;
    assert_eq!(r5515_counters(&driver, "Marvel Boy, Noh-Varr"), 1);
    r5515_power_up(&mut driver, "Jack of Hearts, Volatile Hero")?;
    assert_eq!(r5515_counters(&driver, "Marvel Boy, Noh-Varr"), 2);
    Ok(())
}
