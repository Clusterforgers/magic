//! Goal: abilities given to an exiled card work from exile (Uvilda, Dean of Perfection): the card loses a refine counter
//! in each of its owner's upkeeps, and when the last is removed its owner may cast it for {4} less, a reduction that
//! holds only for the cast its own ability offers, at two to four seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::value::CounterKind;
use mtg_core::color::Color;
use mtg_engine::choose::{CardsChoice, YesNoChoice};
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{creature, instant, island};
use mtg_testkit::{Driver, Scenario, ready};
use rstest::rstest;

use crate::common::pid;

/// Uvilda's ability, as its rules file words it.
const REFINE: &str = include_str!("r112_04_refine.json");

fn uvilda() -> Arc<CardDefinition> {
    let ability: Ability = serde_json::from_str(REFINE).unwrap();
    Arc::new(
        (*creature("Uvilda", 1, &[Color::Blue], 1, 3))
            .clone()
            .with_rules(ability),
    )
}

/// Answers every question of seat 0 yes (the first card, automatic payment), the rest passively, until the turn and
/// step are reached with an empty stack.
fn until(driver: &mut Driver, turn: u32, step: Step) {
    for _ in 0..800 {
        let decision = driver.decision().unwrap().clone();
        let state = driver.state();
        if state.turn().number() == turn
            && state.turn().step() == step
            && matches!(decision.kind, DecisionKind::Priority(_))
            && state.stack().is_empty()
            && decision.player == pid(0)
        {
            return;
        }
        let answer = match &decision.kind {
            DecisionKind::YesNo(_) if decision.player == pid(0) => Answer::YesNo(YesNoChoice(true)),
            DecisionKind::ChooseCards(o) if decision.player == pid(0) => {
                Answer::ChooseCards(CardsChoice(o.from.iter().copied().take(1).collect()))
            }
            DecisionKind::PayMana(_) => Answer::PayMana(PayChoice::Auto),
            _ => Driver::passive_answer(&decision),
        };
        driver.answer(answer).unwrap();
    }
    panic!("turn {turn} {step:?} was never reached");
}

fn thought(driver: &Driver) -> &mtg_engine::object::GameObject {
    driver
        .state()
        .objects()
        .find(|object| object.card().name == "Deep Thought")
        .unwrap()
}

#[rstest]
fn r112_the_refined_card_is_cast_for_four_less_when_its_last_counter_goes(
    #[values(2, 3, 4)] seats: usize,
) {
    let scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .battlefield(0, [ready(uvilda()), ready(island())])
        .hand(0, [instant("Deep Thought", 4, &[Color::Blue])])
        .library(0, (0..6).map(|_| island()).collect::<Vec<_>>());
    let mut driver = Driver::from_scenario(scenario).unwrap();
    driver.begin_activation("Uvilda").unwrap();
    until(&mut driver, 1, Step::Main1);
    let refine = CounterKind::Other("refine".to_owned());
    assert_eq!(thought(&driver).zone(), Zone::Exile);
    assert_eq!(thought(&driver).card_counters_of(&refine), 3);
    let seats = u32::try_from(seats).unwrap();
    until(&mut driver, 1 + seats, Step::Main1);
    assert_eq!(thought(&driver).card_counters_of(&refine), 2);
    until(&mut driver, 1 + 2 * seats, Step::Main1);
    assert_eq!(thought(&driver).card_counters_of(&refine), 1);
    // The last counter goes in the third upkeep: the card is cast with the one Island, {4} less than {4}{U}.
    until(&mut driver, 1 + 3 * seats, Step::Main1);
    assert_eq!(thought(&driver).zone(), Zone::Graveyard(pid(0)));
}
