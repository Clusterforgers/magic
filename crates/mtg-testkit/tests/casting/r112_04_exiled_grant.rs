//! Goal: a triggered ability given to a card in exile works from there: Cosima, God of the Voyage exiles itself in its
//! controller's upkeep and gains "whenever a land you control enters", which puts voyage counters on the exiled card
//! and, once declined, returns it with that many +1/+1 counters and draws that many cards, at two to four seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_cards::value::CounterKind;
use mtg_engine::choose::YesNoChoice;
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{forest, island};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ready};
use rstest::rstest;

use crate::common::pid;

/// Answers yes or no to each "may" of seat 0 from `answers` in turn, the rest passively, until the turn and step are
/// reached with priority.
fn until(driver: &mut Driver, turn: u32, step: Step, answers: &mut Vec<bool>) {
    for _ in 0..600 {
        let state = driver.state();
        let decision = driver.decision().unwrap().clone();
        if state.turn().number() == turn
            && state.turn().step() == step
            && matches!(decision.kind, DecisionKind::Priority(_))
            && state.stack().is_empty()
            && decision.player == pid(0)
        {
            return;
        }
        if matches!(decision.kind, DecisionKind::YesNo(_)) && decision.player == pid(0) {
            let yes = answers.remove(0);
            driver.answer(Answer::YesNo(YesNoChoice(yes))).unwrap();
        } else {
            driver.answer(Driver::passive_answer(&decision)).unwrap();
        }
    }
    panic!("turn {turn} {step:?} was never reached");
}

fn cosima(driver: &Driver) -> &mtg_engine::object::GameObject {
    driver
        .state()
        .objects()
        .find(|object| object.card().name.starts_with("Cosima"))
        .unwrap()
}

/// Lets the land the player plays now resolve its trigger with the answers.
fn land(driver: &mut Driver, name: &str, answers: &mut Vec<bool>) {
    driver.play_land(name).unwrap();
    let turn = driver.state().turn().number();
    until(driver, turn, Step::Main1, answers);
}

#[rstest]
fn r112_cosima_counts_voyages_in_exile_and_returns_with_them(
    #[values(2, 3, 4)] seats: usize,
    #[values(0, 1)] voyages: u32,
) {
    let scenario = Scenario::new(seats)
        .at(1, 0, Step::Untap)
        .battlefield(
            0,
            [ready(
                card("Cosima, God of the Voyage // The Omenkeel").unwrap(),
            )],
        )
        .hand(0, [forest(), island()])
        .library(0, [forest(), forest(), forest(), forest()]);
    let mut driver = Driver::from_scenario(scenario).unwrap();
    // Upkeep: exile Cosima.
    let mut answers = vec![true];
    until(&mut driver, 1, Step::Main1, &mut answers);
    assert_eq!(cosima(&driver).zone(), Zone::Exile);
    let mut turn = 1;
    for _ in 0..voyages {
        land(&mut driver, "Forest", &mut vec![true]);
        assert_eq!(cosima(&driver).zone(), Zone::Exile);
        turn += u32::try_from(seats).unwrap();
        until(&mut driver, turn, Step::Main1, &mut Vec::new());
    }
    assert_eq!(
        cosima(&driver).card_counters_of(&CounterKind::Other("voyage".to_owned())),
        voyages
    );
    let hand = driver.state().zone(Zone::Hand(pid(0))).len();
    let name = if voyages == 0 { "Forest" } else { "Island" };
    land(&mut driver, name, &mut vec![false]);
    let back = cosima(&driver);
    assert_eq!(back.zone(), Zone::Battlefield);
    assert_eq!(
        back.permanent()
            .unwrap()
            .counters_of(&CounterKind::PlusOnePlusOne),
        voyages
    );
    // The land left the hand and each voyage drew a card.
    let drawn = usize::try_from(voyages).unwrap();
    assert_eq!(
        driver.state().zone(Zone::Hand(pid(0))).len(),
        hand - 1 + drawn
    );
}
