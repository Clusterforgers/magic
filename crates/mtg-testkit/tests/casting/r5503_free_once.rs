//! Goal: One with the Multiverse plays at tables of two to four seats: once during each of its controller's turns a spell
//! from the hand or the top of the library is cast without paying its mana cost, and a second free cast that turn is
//! refused whichever zone the first came from.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{bear, elf};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::name_of;

/// Answers until the stack is empty and a player has priority, paying automatically.
fn r5503_resolve(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..64 {
        let decision = driver.decision()?.clone();
        if matches!(decision.kind, DecisionKind::Priority(_))
            && driver.state().zone(Zone::Stack).is_empty()
        {
            return Ok(());
        }
        let answer = match &decision.kind {
            DecisionKind::PayMana(_) => Answer::PayMana(PayChoice::Auto),
            _ => Driver::passive_answer(&decision),
        };
        driver.answer(answer)?;
    }
    Err(ScriptError::Stuck("the stack does not empty".into()))
}

/// How many permanents of that name are on the battlefield.
fn r5503_count(driver: &Driver, name: &str) -> usize {
    let state = driver.state();
    state
        .zone(Zone::Battlefield)
        .iter()
        .filter(|id| name_of(state, **id) == name)
        .count()
}

#[rstest]
#[case::two_seats_hand_first(2, "Grizzly Bears", "Llanowar Elf")]
#[case::three_seats_library_first(3, "Llanowar Elf", "Grizzly Bears")]
#[case::four_seats_hand_first(4, "Grizzly Bears", "Llanowar Elf")]
fn r5503_one_free_spell_each_turn(
    #[case] seats: usize,
    #[case] first: &str,
    #[case] second: &str,
) -> Result<(), ScriptError> {
    // No lands: only the free cast can pay for anything.
    let scenario = Scenario::new(seats)
        .battlefield(0, vec![ready(card("One with the Multiverse").unwrap())])
        .hand(0, vec![bear()])
        .library(0, vec![elf(), elf()]);
    let mut driver = Driver::from_scenario(scenario)?;
    driver.begin_cast(first)?;
    r5503_resolve(&mut driver)?;
    assert_eq!(r5503_count(&driver, first), 1, "{first} was cast free");
    assert!(
        driver.begin_cast(second).is_err(),
        "the free cast is used up"
    );
    assert_eq!(r5503_count(&driver, second), 0);
    Ok(())
}
