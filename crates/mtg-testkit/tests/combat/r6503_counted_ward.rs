//! Goal: a ward whose cost is counted as the trigger resolves ("Minthara has ward {X}, where X is the number of experience counters you have", CR 702.21a) asks the opponent who targets it for that much generic mana, counted from its controller's experience counters then, and counters the spell unless it is paid; with no counters the cost is {0}; at tables of two to four seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common::pid;
use mtg_cards::value::CounterKind;
use mtg_engine::choose::YesNoChoice;
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::turn::Step;
use mtg_testkit::cards::{burn, forest, mountain};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// Seat 1 is active with a Bolt and `lands` Forests and a Mountain; seat 0 controls Minthara and has `experience`
/// experience counters.
fn table(seats: usize, experience: u32, lands: usize) -> Scenario {
    let minthara = card("Minthara, Merciless Soul").unwrap();
    Scenario::new(seats)
        .at(1, 1, Step::Main1)
        .battlefield(0, [ready(minthara)])
        .counters(0, CounterKind::Other("experience".to_owned()), experience)
        .battlefield(
            1,
            [vec![ready(mountain())], vec![ready(forest()); lands]].concat(),
        )
        .hand(1, [burn("Bolt", 9)])
}

fn resolve(driver: &mut Driver) -> Result<(), ScriptError> {
    while matches!(driver.decision()?.kind, DecisionKind::Priority(_))
        && !driver.state().stack().is_empty()
    {
        driver.pass()?;
    }
    Ok(())
}

#[rstest]
#[case::two_seats_pays_two(2, 2, 2, true)]
#[case::three_seats_declines(3, 3, 3, false)]
#[case::four_seats_cannot_pay_three(4, 3, 2, false)]
#[case::four_seats_pays_one(4, 1, 1, true)]
fn ward_x_counts_experience_counters(
    #[case] seats: usize,
    #[case] experience: u32,
    #[case] lands: usize,
    #[case] pays: bool,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table(seats, experience, lands))?;
    let minthara = driver.target_named("Minthara, Merciless Soul")?;
    driver.cast_at("Bolt", &[minthara])?;
    resolve(&mut driver)?;
    let decision = driver.decision()?.clone();
    assert_eq!(decision.player, pid(1));
    let DecisionKind::YesNo(options) = &decision.kind else {
        panic!("the ward asks for its cost");
    };
    assert_eq!(options.text, format!("Pay {{{experience}}}?"));
    driver.answer(Answer::YesNo(YesNoChoice(pays)))?;
    if pays {
        driver.answer(Answer::PayMana(PayChoice::Auto))?;
    }
    resolve(&mut driver)?;
    assert_eq!(
        driver.id_of("Minthara, Merciless Soul").is_ok(),
        !pays,
        "the Bolt resolves only when the ward was paid"
    );
    Ok(())
}
