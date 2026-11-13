//! Goal: a flashback cost that sacrifices or exiles X ("Flashback—{R}{R}, Sacrifice X Mountains.", Firecat Blitz;
//! "Flashback—{1}{U}, Exile X blue cards from your graveyard.", Flash of Insight) announces X from the graveyard, offers only
//! the permanents or cards the filter picks, takes exactly X of them and runs the spell with that X, at two to four seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common::{names, pid};
use mtg_core::color::Color;
use mtg_engine::choose::{CardsChoice, NumberChoice};
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{instant, island, mountain};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError};
use rstest::rstest;

/// Casts `name` from the graveyard with X = `x`, picking the first X offered; the cards offered for the cost.
fn cast(driver: &mut Driver, name: &str, x: u32) -> Result<Vec<String>, ScriptError> {
    driver.begin_cast(name)?;
    let mut offered = Vec::new();
    for _ in 0..12 {
        let decision = driver.decision()?.clone();
        match &decision.kind {
            DecisionKind::ChooseNumber(_) => {
                driver.answer(Answer::ChooseNumber(NumberChoice::Number(x)))?;
            }
            DecisionKind::ChooseCards(options) => {
                offered = options
                    .from
                    .iter()
                    .map(|id| crate::common::name_of(driver.state(), *id))
                    .collect();
                let picked = options.from[..usize::try_from(x).unwrap()].to_vec();
                driver.answer(Answer::ChooseCards(CardsChoice(picked)))?;
            }
            DecisionKind::PayMana(_) => {
                driver.answer(Answer::PayMana(PayChoice::Auto))?;
                break;
            }
            other => panic!("unexpected question {}", other.name()),
        }
    }
    Ok(offered)
}

/// Passes until the stack is empty, answering any question of the resolution with its first choice.
fn resolve(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..30 {
        if driver.state().stack().is_empty() {
            break;
        }
        driver.answer_passively()?;
    }
    Ok(())
}

fn count(list: &[String], name: &str) -> usize {
    list.iter().filter(|n| *n == name).count()
}

#[rstest]
#[case::two_seats_one(2, 1)]
#[case::three_seats_two(3, 2)]
#[case::four_seats_three(4, 3)]
fn firecat_blitz_sacrifices_x_mountains_for_x_cats(
    #[case] seats: usize,
    #[case] x: u32,
) -> Result<(), ScriptError> {
    let mut scenario = Scenario::new(seats)
        .battlefield(
            0,
            (0..6)
                .map(|_| mountain())
                .chain((0..2).map(|_| island()))
                .map(mtg_testkit::scenario::ready),
        )
        .graveyard_named(0, ["Firecat Blitz"]);
    for seat in 0..seats {
        scenario = scenario.library(seat, (0..10).map(|_| island()));
    }
    let mut driver = Driver::from_scenario(scenario)?;
    let offered = cast(&mut driver, "Firecat Blitz", x)?;
    assert!(
        offered.iter().all(|name| name == "Mountain"),
        "only Mountains are offered: {offered:?}"
    );
    resolve(&mut driver)?;
    let board = names(driver.state(), Zone::Battlefield);
    let x = usize::try_from(x).unwrap();
    assert_eq!(count(&board, "Mountain"), 6 - x);
    assert_eq!(count(&board, "Island"), 2);
    assert_eq!(count(&board, "Elemental Cat"), x);
    assert_eq!(names(driver.state(), Zone::Exile), ["Firecat Blitz"]);
    Ok(())
}

#[rstest]
#[case::two_seats_one(2, 1)]
#[case::three_seats_two(3, 2)]
#[case::four_seats_three(4, 3)]
fn flash_of_insight_exiles_x_blue_cards_and_looks_at_x(
    #[case] seats: usize,
    #[case] x: u32,
) -> Result<(), ScriptError> {
    let blue = (0..3).map(|i| instant(&format!("Blue {i}"), 1, &[Color::Blue]));
    let red = (0..2).map(|i| instant(&format!("Red {i}"), 1, &[Color::Red]));
    let mut scenario = Scenario::new(seats)
        .battlefield(
            0,
            (0..4).map(|_| island()).map(mtg_testkit::scenario::ready),
        )
        .graveyard(
            0,
            std::iter::once(card("Flash of Insight").unwrap())
                .chain(blue)
                .chain(red),
        );
    for seat in 0..seats {
        scenario = scenario.library(seat, (0..10).map(|_| mountain()));
    }
    let mut driver = Driver::from_scenario(scenario)?;
    let hand_before = names(driver.state(), Zone::Hand(pid(0))).len();
    let offered = cast(&mut driver, "Flash of Insight", x)?;
    assert_eq!(offered.len(), 3, "the blue cards alone: {offered:?}");
    assert!(offered.iter().all(|name| name.starts_with("Blue")));
    resolve(&mut driver)?;
    let exile = names(driver.state(), Zone::Exile);
    let x = usize::try_from(x).unwrap();
    assert_eq!(exile.iter().filter(|n| n.starts_with("Blue")).count(), x);
    assert!(exile.contains(&"Flash of Insight".to_owned()));
    assert_eq!(
        names(driver.state(), Zone::Hand(pid(0))).len(),
        hand_before + 1,
        "one of the top X cards goes to the hand"
    );
    let graveyard = names(driver.state(), Zone::Graveyard(pid(0)));
    assert_eq!(graveyard.iter().filter(|n| n.starts_with("Red")).count(), 2);
    Ok(())
}
