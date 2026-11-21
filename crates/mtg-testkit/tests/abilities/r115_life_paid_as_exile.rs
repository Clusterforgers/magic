//! Goal: life a player would pay while their library holds at least that many cards is paid by exiling that many cards
//! from its top instead (CR 119.4, 614.1, Ashiok, Wicked Manipulator) at 2 to 4 seats: with a short library, or with
//! the replacement under another player's control, the life is paid; each log replays to the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_cards::value::CounterKind;
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::zones::Zone;
use mtg_testkit::cards::forest;
use mtg_testkit::corpus::card;
use mtg_testkit::scenario::with_counters;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::copying::replays;
use crate::common::pid;

/// Seat `owner` controls Ashiok; seat 0 controls Staff of Compleation and has `library` Forests.
fn table(seats: usize, owner: usize, library: usize) -> Scenario {
    let staff = ready(card("Staff of Compleation").unwrap());
    let ashiok = with_counters(
        ready(card("Ashiok, Wicked Manipulator").unwrap()),
        CounterKind::Loyalty,
        5,
    );
    let scenario =
        Scenario::new(seats).library(0, (0..library).map(|_| forest()).collect::<Vec<_>>());
    if owner == 0 {
        scenario.battlefield(0, [staff, ashiok])
    } else {
        scenario
            .battlefield(0, [staff])
            .battlefield(owner, [ashiok])
    }
}

/// Activates "{T}, Pay 4 life: Draw a card." and plays on until the stack is empty.
fn draw_for_life(driver: &mut Driver) -> Result<(), ScriptError> {
    driver.begin_ability("Staff of Compleation", 3)?;
    for _ in 0..100 {
        let decision = driver.decision()?.clone();
        match &decision.kind {
            DecisionKind::Priority(_) if driver.state().stack().is_empty() => return Ok(()),
            DecisionKind::PayMana(_) => {
                driver.answer(Answer::PayMana(PayChoice::Auto))?;
            }
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    Err(ScriptError::Stuck("the stack".into()))
}

// (owner of Ashiok, library, life after, cards exiled, library after)
#[rstest]
#[case::exiled_instead(0, 6, 20, 4, 1)]
#[case::exactly_enough(0, 5, 20, 4, 0)]
#[case::short_library(0, 3, 16, 0, 2)]
#[case::an_opponents_ashiok(1, 6, 16, 0, 5)]
fn life_paid_while_the_library_holds_enough_is_exiled_cards(
    #[values(2, 3, 4)] seats: usize,
    #[case] owner: usize,
    #[case] library: usize,
    #[case] life: i64,
    #[case] exiled: usize,
    #[case] left: usize,
) -> Result<(), ScriptError> {
    let setup = table(seats, owner, library).setup().unwrap();
    let mut driver = Driver::from_scenario(table(seats, owner, library))?;
    draw_for_life(&mut driver)?;
    assert_eq!(
        i64::from(driver.state().player(pid(0)).unwrap().life()),
        life
    );
    assert_eq!(driver.state().zone(Zone::Exile).len(), exiled);
    assert_eq!(driver.state().zone(Zone::Library(pid(0))).len(), left);
    replays(&driver, &setup);
    Ok(())
}
