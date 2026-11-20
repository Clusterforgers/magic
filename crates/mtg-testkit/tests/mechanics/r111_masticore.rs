//! Goal: "sacrifice this creature unless you discard a card. When you discard a card this way, destroy target nonland
//! permanent an opponent controls with mana value less than or equal to the mana value of the discarded card."
//! (Argentum Masticore) at 2 to 4 seats: a discard sets off the reflexive trigger, whose targets are read from the
//! discarded card's mana value; declining sacrifices the Masticore and sets off nothing.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_core::color::Color;
use mtg_engine::choose::{CardsChoice, YesNoChoice};
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{creature, forest};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ready};
use rstest::rstest;

use crate::common::{names, pid};

/// Seat 0 controls the Masticore at its upkeep and holds a creature card of mana value `discarded`; seat 1 controls a
/// creature of mana value 2 and one of mana value 4.
fn game(seats: usize, discarded: u32) -> Driver {
    let mut scenario = Scenario::new(seats)
        .at(2, 0, Step::Untap)
        .hand(0, vec![creature("Fodder", discarded, &[], 1, 1)])
        .battlefield(0, vec![ready(card("Argentum Masticore").unwrap())])
        .battlefield(
            1,
            vec![
                ready(creature("Two Drop", 1, &[Color::Green], 2, 2)),
                ready(creature("Four Drop", 3, &[Color::Green], 4, 4)),
            ],
        );
    for seat in 0..seats {
        scenario = scenario.library(seat, vec![forest(); 8]);
    }
    Driver::from_scenario(scenario).unwrap()
}

/// Answers until the stack is empty: says `discard` to the payment, discards the first card offered and records the
/// legal targets of the reflexive trigger, choosing the first. Returns the names of the targets offered.
fn settle(driver: &mut Driver, discard: bool) -> Vec<String> {
    let mut offered = Vec::new();
    for _ in 0..40 {
        let decision = driver.decision().unwrap().clone();
        match &decision.kind {
            DecisionKind::Priority(_)
                if driver.state().stack().is_empty()
                    && driver.state().turn().step() == Step::Main1 =>
            {
                break;
            }
            DecisionKind::YesNo(_) => {
                driver.answer(Answer::YesNo(YesNoChoice(discard))).unwrap();
            }
            DecisionKind::ChooseCards(options) => {
                let chosen = options.from.iter().copied().take(1).collect();
                driver
                    .answer(Answer::ChooseCards(CardsChoice(chosen)))
                    .unwrap();
            }
            DecisionKind::ChooseTargets(options) => {
                offered = options
                    .legal
                    .iter()
                    .filter_map(|target| target.object())
                    .map(|object| {
                        driver
                            .state()
                            .object(object.id)
                            .unwrap()
                            .card()
                            .name
                            .clone()
                    })
                    .collect();
                let first = options.legal.iter().take(1).copied().collect::<Vec<_>>();
                driver.choose_targets(&first).unwrap();
            }
            _ => {
                driver.answer_passively().unwrap();
            }
        }
    }
    offered.sort();
    offered
}

#[rstest]
#[case::three(3, &["Two Drop"])]
#[case::four(4, &["Four Drop", "Two Drop"])]
#[case::one(1, &[])]
fn a_discard_destroys_a_permanent_of_at_most_its_mana_value(
    #[values(2, 3, 4)] seats: usize,
    #[case] discarded: u32,
    #[case] offered: &[&str],
) {
    let mut driver = game(seats, discarded);
    assert_eq!(settle(&mut driver, true), offered);
    let field = names(driver.state(), Zone::Battlefield);
    assert!(field.iter().any(|name| name == "Argentum Masticore"));
    assert!(names(driver.state(), Zone::Graveyard(pid(0))).contains(&"Fodder".to_owned()));
    // The target chosen is destroyed: one of those offered is gone.
    let gone = offered
        .iter()
        .filter(|name| !field.iter().any(|held| held == *name))
        .count();
    assert_eq!(gone, usize::from(!offered.is_empty()));
}

#[rstest]
fn declining_sacrifices_the_masticore(#[values(2, 3, 4)] seats: usize) {
    let mut driver = game(seats, 4);
    assert_eq!(settle(&mut driver, false), Vec::<String>::new());
    let graveyard = names(driver.state(), Zone::Graveyard(pid(0)));
    assert_eq!(graveyard, ["Argentum Masticore"]);
}
