//! Goal: "Return target spell to its owner's hand." (Reprieve) takes the spell off the stack into its owner's hand
//! uncast and unresolved, whoever controls it, and the rest of the spell goes on (the card is drawn); each log replays
//! to the live state; at two to four seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_engine::choose::Target;
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{forest, free_creature, plains};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::copying::replays;
use crate::common::{names, pid};

/// Seat 1, active, holds Grizzly Bears with two Forests; seat 0 holds Reprieve with two Plains.
fn table(seats: usize) -> Scenario {
    let mut scenario = Scenario::new(seats)
        .at(2, 1, Step::Main1)
        .battlefield(0, [ready(plains()), ready(plains())])
        .hand(0, [card("Reprieve").unwrap()])
        .battlefield(1, [ready(forest()), ready(forest())])
        .hand(1, [card("Grizzly Bears").unwrap()]);
    for seat in 0..seats {
        scenario = scenario.library(seat, (0..6).map(|_| free_creature("Filler", 1, 1)));
    }
    scenario
}

#[rstest]
fn p158_reprieve_returns_the_spell_to_its_owners_hand(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let setup = table(seats).setup().unwrap();
    let mut driver = Driver::from_scenario(table(seats))?;
    driver.cast("Grizzly Bears")?;
    while driver.decision()?.player != pid(0) {
        driver.pass()?;
    }
    let spell = driver.state().zone(Zone::Stack)[0];
    let bears = Target::Object(driver.state().object(spell).unwrap().reference());
    driver.cast_at("Reprieve", &[bears])?;
    for _ in 0..200 {
        let decision = driver.decision()?.clone();
        match &decision.kind {
            DecisionKind::Priority(_) if driver.state().stack().is_empty() => break,
            DecisionKind::PayMana(_) => {
                driver.answer(Answer::PayMana(PayChoice::Auto))?;
            }
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    assert_eq!(names(driver.state(), Zone::Hand(pid(1))), ["Grizzly Bears"]);
    assert!(!names(driver.state(), Zone::Battlefield).contains(&"Grizzly Bears".to_owned()));
    assert_eq!(names(driver.state(), Zone::Hand(pid(0))), ["Filler"]);
    assert_eq!(names(driver.state(), Zone::Graveyard(pid(0))), ["Reprieve"]);
    replays(&driver, &setup);
    Ok(())
}
