//! Goal: a spell's own cost is reduced for each thing its "any number of" additional cost took (Hierophant Bio-Titan:
//! {2} less for each +1/+1 counter removed from among creatures you control) at 2 and 3 seats: X is asked from 0 to the
//! counters of the caster's own creatures, the counters go and the mana asked falls by {2} for each.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_cards::value::CounterKind;
use mtg_core::mana::ManaCost;
use mtg_engine::choose::NumberChoice;
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{forest, free_creature};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready, with_counters};
use rstest::rstest;

use crate::common::casting::{lands, pay};
use crate::common::pid;

const TITAN: &str = "Hierophant Bio-Titan";

/// Seat 0 holds the Titan, `forests` Forests and two creatures with 2 and 3 +1/+1 counters; the next seat has a creature
/// with 4 counters the caster cannot take.
fn table(seats: usize, forests: usize) -> Scenario {
    let counted = |name: &str, count: u32| {
        with_counters(
            ready(free_creature(name, 1, 1)),
            CounterKind::PlusOnePlusOne,
            count,
        )
    };
    Scenario::new(seats)
        .hand(0, [card(TITAN).unwrap()])
        .battlefield(
            0,
            lands(forests, forest)
                .into_iter()
                .chain([counted("Two", 2), counted("Three", 3)]),
        )
        .battlefield(1, [counted("Theirs", 4)])
}

fn counters_left(driver: &Driver, seat: usize) -> u32 {
    driver
        .state()
        .zone(Zone::Battlefield)
        .iter()
        .filter_map(|id| driver.state().object(*id))
        .filter(|object| object.controller() == pid(seat))
        .filter_map(|object| object.permanent())
        .map(|permanent| permanent.counters_of(&CounterKind::PlusOnePlusOne))
        .sum()
}

#[rstest]
fn each_counter_removed_makes_the_titan_cost_two_less(
    #[values(0, 2, 5)] x: u32,
    #[values(2, 3)] seats: usize,
) -> Result<(), ScriptError> {
    let forests = 12 - 2 * usize::try_from(x).unwrap();
    let mut driver = Driver::from_scenario(table(seats, forests))?;
    driver.begin_cast(TITAN)?;
    let DecisionKind::ChooseNumber(options) = &driver.decision()?.kind else {
        panic!("X is asked: {:?}", driver.decision()?.kind);
    };
    assert_eq!(
        (options.min, options.max),
        (0, Some(5)),
        "only the caster's counters"
    );
    driver.answer(Answer::ChooseNumber(NumberChoice::Number(x)))?;
    let generic = 10 - 2 * x;
    assert_eq!(
        driver.pay_options()?.cost,
        ManaCost::parse(&format!("{{{generic}}}{{G}}{{G}}")).unwrap()
    );
    pay(&mut driver)?;
    assert_eq!(counters_left(&driver, 0), 5 - x);
    assert_eq!(
        counters_left(&driver, 1),
        4,
        "the other seat keeps its counters"
    );
    assert_eq!(driver.state().stack().len(), 1, "the Titan is cast");
    Ok(())
}
