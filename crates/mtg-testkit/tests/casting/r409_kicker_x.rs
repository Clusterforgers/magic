//! Goal: a kicker with `{X}` plays whole at two to four seats: X is asked only once the spell is kicked, offered up to what
//! the lands can pay beside the rest of the cost, the total holds X, and the permanent's "if it was kicked" trigger reads
//! that X (Verdeloth the Ancient makes X Saprolings); an unkicked cast asks no X and does not trigger.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use common::names;
use mtg_engine::choose::{CostsChoice, NumberChoice};
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::zones::Zone;
use mtg_testkit::cards::forest;
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// `seats` players; seat 0 holds Verdeloth the Ancient and controls `forests` Forests.
fn r409_table(seats: usize, forests: usize) -> Scenario {
    Scenario::new(seats)
        .hand(0, [card("Verdeloth the Ancient").unwrap()])
        .battlefield(0, (0..forests).map(|_| ready(forest())).collect::<Vec<_>>())
}

fn r409_count(driver: &Driver, name: &str) -> usize {
    names(driver.state(), Zone::Battlefield)
        .iter()
        .filter(|found| *found == name)
        .count()
}

/// Every player passes until the stack is empty.
fn r409_resolve_all(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..40 {
        if driver.state().zone(Zone::Stack).is_empty() {
            return Ok(());
        }
        driver.pass()?;
    }
    panic!("the stack never empties");
}

#[rstest]
#[case::kicked_for_three(2, 9, Some(3), 3)]
#[case::kicked_for_the_most(3, 8, Some(2), 2)]
#[case::kicked_for_zero(4, 7, Some(0), 0)]
#[case::not_kicked(2, 9, None, 0)]
#[case::not_kicked_four_seats(4, 6, None, 0)]
fn r409_kicker_x_makes_x_saprolings(
    #[case] seats: usize,
    #[case] forests: usize,
    #[case] x: Option<u32>,
    #[case] saprolings: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(r409_table(seats, forests))?;
    driver.begin_cast("Verdeloth the Ancient")?;
    driver.expect_decision(0, "choose-costs")?;
    let kicked = if x.is_some() { vec![0] } else { Vec::new() };
    driver.answer(Answer::ChooseCosts(CostsChoice::Costs(kicked)))?;
    if let Some(x) = x {
        let DecisionKind::ChooseNumber(options) = &driver.decision()?.kind else {
            panic!("a kicked Verdeloth asks X");
        };
        let most = u32::try_from(forests).unwrap() - 6;
        assert_eq!((options.min, options.max), (0, Some(most)));
        assert_eq!(options.per_x, 1);
        driver.answer(Answer::ChooseNumber(NumberChoice::Number(x)))?;
    }
    let paid = driver.pay_options()?.cost.mana_value();
    assert_eq!(paid, 6 + x.unwrap_or(0));
    driver.expect_decision(0, "pay-mana")?;
    driver.answer(Answer::PayMana(PayChoice::Auto))?;
    r409_resolve_all(&mut driver)?;
    assert_eq!(r409_count(&driver, "Verdeloth the Ancient"), 1);
    assert_eq!(r409_count(&driver, "Saproling"), saprolings);
    Ok(())
}

/// `seats` players; seat 0 holds Thieving Skydiver and controls `islands` Islands, seat 1 controls Bonesplitter and Sol
/// Ring.
fn r409_skydiver_table(seats: usize, islands: usize) -> Scenario {
    Scenario::new(seats)
        .hand(0, [card("Thieving Skydiver").unwrap()])
        .battlefield(
            0,
            (0..islands)
                .map(|_| ready(mtg_testkit::cards::island()))
                .collect::<Vec<_>>(),
        )
        .battlefield(
            1,
            [
                ready(card("Bonesplitter").unwrap()),
                ready(card("Sol Ring").unwrap()),
            ],
        )
}

/// Thieving Skydiver kicked for X takes an artifact of mana value X or less and wears it when it is an Equipment; X is at
/// least 1 ("X can't be 0"), and an unkicked Skydiver takes nothing.
#[rstest]
#[case::takes_the_equipment(2, 4, Some(1), Some("Bonesplitter"), true)]
#[case::takes_the_ring(3, 5, Some(2), Some("Sol Ring"), false)]
#[case::four_seats(4, 4, Some(1), Some("Bonesplitter"), true)]
#[case::not_kicked(2, 4, None, None, false)]
fn r409_skydiver_takes_an_artifact(
    #[case] seats: usize,
    #[case] islands: usize,
    #[case] x: Option<u32>,
    #[case] taken: Option<&str>,
    #[case] attached: bool,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(r409_skydiver_table(seats, islands))?;
    driver.begin_cast("Thieving Skydiver")?;
    driver.expect_decision(0, "choose-costs")?;
    let kicked = if x.is_some() { vec![0] } else { Vec::new() };
    driver.answer(Answer::ChooseCosts(CostsChoice::Costs(kicked)))?;
    if let Some(x) = x {
        let DecisionKind::ChooseNumber(options) = &driver.decision()?.kind else {
            panic!("a kicked Skydiver asks X");
        };
        let most = u32::try_from(islands).unwrap() - 2;
        assert_eq!((options.min, options.max), (1, Some(most)), "X can't be 0");
        driver.answer(Answer::ChooseNumber(NumberChoice::Number(x)))?;
    }
    driver.expect_decision(0, "pay-mana")?;
    driver.answer(Answer::PayMana(PayChoice::Auto))?;
    driver.pass()?;
    for _ in 1..seats {
        if !driver.state().zone(Zone::Stack).is_empty() {
            driver.pass()?;
        }
    }
    if let Some(name) = taken {
        let target = driver.target_named(name)?;
        driver.choose_targets(&[target])?;
    }
    r409_resolve_all(&mut driver)?;
    let you = mtg_core::ids::PlayerId::from_index(0);
    for name in ["Bonesplitter", "Sol Ring"] {
        let id = driver.id_of(name)?;
        let controller = mtg_engine::characteristics::controller_of(driver.state(), id);
        assert_eq!(controller == Some(you), taken == Some(name), "{name}");
    }
    let bonesplitter = driver.id_of("Bonesplitter")?;
    let worn = driver
        .state()
        .object(bonesplitter)
        .and_then(|object| object.permanent())
        .is_some_and(|permanent| permanent.attached_to().is_some());
    assert_eq!(worn, attached);
    Ok(())
}
