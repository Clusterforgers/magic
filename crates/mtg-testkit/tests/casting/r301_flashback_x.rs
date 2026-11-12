//! Goal: a flashback cost that counts X ("Flashback—{R}{R}, Discard X cards.", Conflagrate) asks X from the graveyard, offers no more than the cards in hand, discards that many and deals X damage, at tables of two to four seats; cast from the hand the same card pays {X}{X} with nothing discarded.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common::casting::lands;
use crate::common::{names, pid};
use mtg_engine::choose::NumberChoice;
use mtg_engine::choose::{CardsChoice, Target, TargetChoice};
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{island, mountain};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError};
use rstest::rstest;

/// Seat 0 with Conflagrate in the graveyard (or hand), Mountains and `hand` Islands in hand.
fn table(seats: usize, hand: usize, from_hand: bool) -> Scenario {
    let islands = (0..hand).map(|_| island());
    let mut scenario = Scenario::new(seats).battlefield(0, lands(6, mountain));
    scenario = if from_hand {
        scenario.hand(
            0,
            std::iter::once(card("Conflagrate").unwrap()).chain(islands),
        )
    } else {
        scenario
            .hand(0, islands)
            .graveyard_named(0, ["Conflagrate"])
    };
    for seat in 0..seats {
        scenario = scenario.library(seat, (0..10).map(|_| island()));
    }
    scenario
}

/// Casts Conflagrate with X = `x` at the last seat, answering each question of the announcement; the most X offered.
fn cast(driver: &mut Driver, seats: usize, x: u32) -> Result<Option<u32>, ScriptError> {
    driver.begin_cast("Conflagrate")?;
    let mut most = None;
    for _ in 0..12 {
        let decision = driver.decision()?.clone();
        match &decision.kind {
            DecisionKind::ChooseNumber(options) => {
                most = options.max;
                driver.answer(Answer::ChooseNumber(NumberChoice::Number(x)))?;
            }
            DecisionKind::ChooseTargets(_) => {
                let at = Target::Player(pid(seats - 1));
                driver.answer(Answer::ChooseTargets(TargetChoice::Targets(vec![at])))?;
            }
            DecisionKind::ChooseCards(options) => {
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
    Ok(most)
}

fn resolve(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..20 {
        if driver.state().stack().is_empty() {
            break;
        }
        driver.answer_passively()?;
    }
    Ok(())
}

#[rstest]
#[case::two_seats_two_cards(2, 2, 2)]
#[case::three_seats_one_of_three(3, 3, 1)]
#[case::four_seats_three_cards(4, 3, 3)]
fn flashback_discards_x_cards_and_deals_x_damage(
    #[case] seats: usize,
    #[case] hand: usize,
    #[case] x: u32,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table(seats, hand, false))?;
    let before = driver.state().player(pid(seats - 1)).unwrap().life();
    let most = cast(&mut driver, seats, x)?;
    assert_eq!(
        most,
        Some(u32::try_from(hand).unwrap()),
        "X is held to the hand"
    );
    assert_eq!(
        names(driver.state(), Zone::Hand(pid(0))).len(),
        hand - usize::try_from(x).unwrap()
    );
    resolve(&mut driver)?;
    let life = driver.state().player(pid(seats - 1)).unwrap().life();
    assert_eq!(before - life, i32::try_from(x).unwrap());
    assert_eq!(names(driver.state(), Zone::Exile), ["Conflagrate"]);
    Ok(())
}

#[test]
fn cast_from_the_hand_x_is_paid_in_mana_and_nothing_is_discarded() -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table(2, 2, true))?;
    cast(&mut driver, 2, 2)?;
    assert_eq!(names(driver.state(), Zone::Hand(pid(0))).len(), 2);
    resolve(&mut driver)?;
    assert_eq!(
        names(driver.state(), Zone::Graveyard(pid(0))),
        ["Conflagrate"]
    );
    Ok(())
}
