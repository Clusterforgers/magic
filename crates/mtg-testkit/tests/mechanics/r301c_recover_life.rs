//! Goal: "Recover—Pay half your life, rounded up." asks its owner to pay half their life, rounded up, when another creature of theirs dies, returns the card to the hand when paid and exiles it otherwise; at 2 to 4 seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common::{names, pid};
use mtg_core::color::Color;
use mtg_engine::choose::YesNoChoice;
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::zones::Zone;
use mtg_testkit::cards::land;
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// Answers every question until the stack is empty: yes to a payment when `pays`; the questions asked.
fn settle(driver: &mut Driver, pays: bool) -> Result<Vec<String>, ScriptError> {
    let mut asked = Vec::new();
    for _ in 0..200 {
        match &driver.decision()?.clone().kind {
            DecisionKind::YesNo(question) => {
                asked.push(question.text.clone());
                let answer = pays && !question.only_no;
                driver.answer(Answer::YesNo(YesNoChoice(answer)))?;
            }
            DecisionKind::PayMana(_) => {
                driver.answer(Answer::PayMana(PayChoice::Auto))?;
            }
            DecisionKind::Priority(_) if driver.state().stack().is_empty() => return Ok(asked),
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    Err(ScriptError::Stuck("an empty stack".into()))
}

#[rstest]
#[case::paid_at_fifteen_life(15, true, 7)]
#[case::paid_at_twenty_life(20, true, 10)]
#[case::declined(15, false, 15)]
fn garzas_assassin_recovers_for_half_your_life_rounded_up(
    #[values(2, 3, 4)] seats: usize,
    #[case] start: i32,
    #[case] pays: bool,
    #[case] left: i32,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(seats)
            .life(0, start)
            .graveyard(0, [card("Garza's Assassin").unwrap()])
            .hand(0, [card("Lightning Bolt").unwrap()])
            .battlefield(
                0,
                [
                    ready(land(Color::Red)),
                    ready(card("Grizzly Bears").unwrap()),
                ],
            ),
    )?;
    let bears = driver.target_named("Grizzly Bears")?;
    driver.cast_at("Lightning Bolt", &[bears])?;
    let asked = settle(&mut driver, pays)?;
    assert_eq!(asked.len(), 1, "{asked:?}");
    assert_eq!(driver.state().player(pid(0)).unwrap().life(), left);
    let in_hand =
        names(driver.state(), Zone::Hand(pid(0))).contains(&"Garza's Assassin".to_owned());
    let exiled = names(driver.state(), Zone::Exile).contains(&"Garza's Assassin".to_owned());
    assert_eq!((in_hand, exiled), (pays, !pays));
    Ok(())
}
