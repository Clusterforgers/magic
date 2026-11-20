//! Goal: a number chosen as an additional cost (`CostItem::R110ChooseNumber`, Liquid Fire) is asked as the spell is cast,
//! only between the numbers the card names, and the resolving spell reads it as X: the target creature is dealt X damage
//! and its controller 5 minus X, at tables of two to four seats and for every end of the range.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_engine::choose::NumberChoice;
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::turn::Step;
use mtg_testkit::cards::{free_creature, mountain};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ready};
use rstest::rstest;

use crate::common::pid;

/// Seat 0 holds Liquid Fire and six Mountains; the last seat controls a 1/10 creature.
fn r110_table(seats: usize) -> Driver {
    let last = seats - 1;
    let scenario = Scenario::new(seats)
        .at(2, 0, Step::Main1)
        .battlefield(0, (0..6).map(|_| ready(mountain())))
        .battlefield(last, [ready(free_creature("Wall Target", 1, 10))])
        .hand(0, [card("Liquid Fire").unwrap()]);
    Driver::from_scenario(scenario).unwrap()
}

#[rstest]
fn r110_the_chosen_number_divides_the_damage(
    #[values(2, 3, 4)] seats: usize,
    #[values(0, 3, 5)] chosen: u32,
) {
    let mut driver = r110_table(seats);
    let last = seats - 1;
    let before = driver.state().player(pid(last)).unwrap().life();
    driver.begin_cast("Liquid Fire").unwrap();
    let DecisionKind::ChooseNumber(options) = driver.decision().unwrap().kind.clone() else {
        panic!("the number is asked first");
    };
    assert_eq!((options.min, options.max), (0, Some(5)));
    assert_eq!(options.text, "Choose a number between 0 and 5.");
    assert_eq!(options.per_x, 0, "the number costs no mana");
    driver
        .answer(Answer::ChooseNumber(NumberChoice::Number(chosen)))
        .unwrap();
    let target = driver.target_named("Wall Target").unwrap();
    driver.choose_targets(&[target]).unwrap();
    driver
        .answer(Answer::PayMana(mtg_engine::decision::PayChoice::Auto))
        .unwrap();
    while !driver.state().stack().is_empty() {
        driver.pass().unwrap();
    }
    let wall = driver.id_of("Wall Target").unwrap();
    let damage = driver
        .state()
        .object(wall)
        .and_then(|object| object.permanent())
        .map(mtg_engine::object::PermanentState::damage);
    assert_eq!(damage, Some(chosen));
    let life = driver.state().player(pid(last)).unwrap().life();
    assert_eq!(i64::from(before) - i64::from(life), i64::from(5 - chosen));
}

#[rstest]
fn r110_a_number_out_of_the_range_is_refused(#[values(2, 4)] seats: usize) {
    let mut driver = r110_table(seats);
    driver.begin_cast("Liquid Fire").unwrap();
    assert!(
        driver
            .answer(Answer::ChooseNumber(NumberChoice::Number(6)))
            .is_err()
    );
}
