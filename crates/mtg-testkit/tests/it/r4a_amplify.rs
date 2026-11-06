//! Goal: amplify (CR 702.38) reveals the creature cards of the hand that share a creature type with the entering creature, puts the counters for each, shows the reveal to every seat, asks nothing when no card qualifies, and counts a changeling as sharing; at tables of two to four seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::keyword::Keyword;
use mtg_cards::value::CounterKind;
use mtg_engine::choose::CardsChoice;
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_testkit::cards::{free_creature, plains};
use mtg_testkit::{Driver, Scenario};
use rstest::rstest;

const AMPLIFY: &str = r#"{"Replacement":{"when":{"Enters":{"what":["This"]}},"by":{"Amplify":2}}}"#;

fn hawk() -> Arc<CardDefinition> {
    let card = (*free_creature("Hawk", 2, 2)).clone();
    Arc::new(
        card.with_subtype("Bird")
            .with_rules(serde_json::from_str::<Ability>(AMPLIFY).unwrap()),
    )
}

fn kin(name: &str, kind: &str) -> Arc<CardDefinition> {
    Arc::new((*free_creature(name, 1, 1)).clone().with_subtype(kind))
}

fn shifter() -> Arc<CardDefinition> {
    let card = (*free_creature("Shifter", 1, 1)).clone();
    Arc::new(card.with_rules(Ability::Keyword(Keyword::Changeling)))
}

fn table(seats: usize, hand: Vec<Arc<CardDefinition>>) -> Driver {
    let mut scenario = Scenario::new(seats).hand(0, hand);
    for seat in 0..seats {
        scenario = scenario.library(seat, (0..16).map(|_| plains()));
    }
    Driver::from_scenario(scenario).unwrap()
}

/// The card named `name` in the hand of seat 0.
fn in_hand(driver: &Driver, name: &str) -> mtg_core::ids::ObjectId {
    driver
        .state()
        .zone(mtg_engine::zones::Zone::Hand(mtg_core::ids::PlayerId(0)))
        .iter()
        .copied()
        .find(|id| {
            driver
                .state()
                .object(*id)
                .is_some_and(|o| o.card().name == name)
        })
        .unwrap()
}

/// Casts the hawk; the reveal question, if any, is answered with these names.
fn enter(driver: &mut Driver, reveal: &[&str]) -> bool {
    driver.cast("Hawk").unwrap();
    let mut asked = false;
    for _ in 0..24 {
        if matches!(
            driver.decision().unwrap().kind,
            DecisionKind::ChooseCards(_)
        ) {
            asked = true;
            let cards = reveal.iter().map(|name| in_hand(driver, name)).collect();
            driver
                .answer(Answer::ChooseCards(CardsChoice(cards)))
                .unwrap();
        } else if driver.state().stack().is_empty() {
            return asked;
        } else {
            driver.pass().unwrap();
        }
    }
    panic!("the hawk never settled");
}

fn counters(driver: &Driver) -> u32 {
    let id = driver.id_of("Hawk").unwrap();
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

#[rstest]
fn each_card_revealed_gives_the_counters(#[values(2, 3, 4)] seats: usize) {
    let hand = vec![
        hawk(),
        kin("Gull", "Bird"),
        kin("Gull Two", "Bird"),
        kin("Wolf", "Wolf"),
    ];
    let mut driver = table(seats, hand);
    assert!(enter(&mut driver, &["Gull", "Gull Two"]));
    assert_eq!(counters(&driver), 4);
}

#[rstest]
fn revealing_none_gives_no_counters(#[values(2, 3)] seats: usize) {
    let mut driver = table(seats, vec![hawk(), kin("Gull", "Bird")]);
    assert!(enter(&mut driver, &[]));
    assert_eq!(counters(&driver), 0);
}

#[rstest]
fn a_card_sharing_no_type_is_not_offered_and_nobody_is_asked(#[values(2, 3)] seats: usize) {
    let mut driver = table(seats, vec![hawk(), kin("Wolf", "Wolf")]);
    assert!(!enter(&mut driver, &[]));
    assert_eq!(counters(&driver), 0);
}

#[test]
fn a_changeling_shares_a_type_with_every_creature() {
    let mut driver = table(2, vec![hawk(), shifter()]);
    assert!(enter(&mut driver, &["Shifter"]));
    assert_eq!(counters(&driver), 2);
}
