//! Goal: Unexpectedly Absent plays at tables of two to four seats as its rules say: the target goes into its owner's library with exactly X cards above it, on top for X = 0 and at the bottom when the library holds fewer than X cards.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_engine::choose::NumberChoice;
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{free_creature, plains};
use mtg_testkit::corpus::corpus;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::pid;

/// Seat 0 holds Unexpectedly Absent and plenty of Plains; the last seat controls a Bear and has a library of ten named
/// cards.
fn rmt1_table(seats: usize) -> Scenario {
    let absent = corpus().unwrap().card("Unexpectedly Absent").unwrap();
    let last = seats - 1;
    let library: Vec<_> = (0..10)
        .map(|at| free_creature(&format!("Card {at}"), 1, 1))
        .collect();
    Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .battlefield(0, (0..20).map(|_| ready(plains())).collect::<Vec<_>>())
        .hand(0, [absent])
        .battlefield(last, [ready(free_creature("Bear", 2, 2))])
        .library(last, library)
}

#[rstest]
#[case::on_top(0, 0)]
#[case::beneath_two(2, 2)]
#[case::at_the_bottom_of_a_short_library(15, 10)]
fn rmt1_the_target_has_x_cards_above_it(
    #[case] x: u32,
    #[case] above: usize,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let last = seats - 1;
    let mut driver = Driver::from_scenario(rmt1_table(seats))?;
    driver.begin_cast("Unexpectedly Absent")?;
    let bear = driver.target_named("Bear")?;
    for _ in 0..10 {
        match &driver.decision()?.kind {
            DecisionKind::ChooseNumber(_) => {
                driver.answer(Answer::ChooseNumber(NumberChoice::Number(x)))?;
            }
            DecisionKind::ChooseTargets(_) => {
                driver.choose_targets(&[bear])?;
            }
            DecisionKind::PayMana(_) => {
                driver.answer(Answer::PayMana(PayChoice::Auto))?;
                break;
            }
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    for _ in 0..40 {
        if driver.state().stack().is_empty() && driver.state().resolving().is_none() {
            break;
        }
        driver.answer_passively()?;
    }
    let state = driver.state();
    let library = state.zone(Zone::Library(pid(last)));
    // The top of a library is the end of its list.
    let at = library
        .iter()
        .position(|id| state.object(*id).is_some_and(|o| o.card().name == "Bear"))
        .expect("the Bear is in its owner's library");
    assert_eq!(library.len() - 1 - at, above);
    Ok(())
}
