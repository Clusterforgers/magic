//! Goal: the r30-2b answers to a spell on the stack play as printed at 2 to 4 seats: Invert Polarity takes the spell
//! on a won flip and counters it on a lost one, Wyll's Reversal points the spell elsewhere and copies it only on a
//! total of 15 or more, and Disrupting Shoal counters a spell only when its mana value is X, cast for its mana or by
//! exiling a blue card of mana value X.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_core::color::Color;
use mtg_engine::choose::{
    CardsChoice, NumberChoice, OptionChoice, Target, TargetChoice, YesNoChoice,
};
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_testkit::cards::{burn, creature, instant, island, mountain};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::casting::ways;
use crate::common::pid;

/// Passes until the stack is empty, aiming every new target at `aim`, answering yes and paying automatically.
fn settle(driver: &mut Driver, aim: Target) -> Result<(), ScriptError> {
    for _ in 0..60 {
        let decision = driver.decision()?.clone();
        if matches!(decision.kind, DecisionKind::Priority(_)) && driver.state().stack().is_empty() {
            return Ok(());
        }
        let answer = match &decision.kind {
            DecisionKind::Priority(_) => {
                driver.pass()?;
                continue;
            }
            DecisionKind::YesNo(o) => Answer::YesNo(YesNoChoice(!o.only_no)),
            DecisionKind::ChooseTargets(o) => {
                let chosen = Some(aim)
                    .filter(|t| o.legal.contains(t))
                    .or_else(|| o.legal.first().copied());
                Answer::ChooseTargets(TargetChoice::Targets(chosen.into_iter().collect()))
            }
            DecisionKind::PayMana(_) => Answer::PayMana(PayChoice::Auto),
            _ => Driver::passive_answer(&decision),
        };
        driver.answer(answer)?;
    }
    Err(ScriptError::Stuck("the stack".into()))
}

/// Seat 0 casts a Bolt at seat 1, and seat 1 answers it with `answer`; the life of both seats once all resolved.
fn bolt_answered(
    seats: usize,
    seed: u64,
    answer: &str,
    power: Option<i32>,
) -> Result<(i32, i32), ScriptError> {
    let lands = (0..3).flat_map(|_| [ready(island()), ready(mountain())]);
    let mut driver = Driver::from_scenario(
        Scenario::new(seats)
            .seed(seed)
            .hand(0, [burn("Bolt", 3)])
            .battlefield(0, [ready(mountain())])
            .hand(1, [card(answer).unwrap()])
            .battlefield(
                1,
                lands
                    .chain(power.map(|power| ready(creature("Titan", 1, &[Color::Red], power, 1)))),
            ),
    )?;
    driver.begin_cast("Bolt")?;
    driver.answer(Answer::ChooseTargets(TargetChoice::Targets(vec![
        Target::Player(pid(1)),
    ])))?;
    driver.answer(Answer::PayMana(PayChoice::Auto))?;
    let on_stack = driver.state().stack()[0];
    let at = Target::Object(driver.state().object(on_stack).unwrap().reference());
    while driver.decision()?.player != pid(1) {
        driver.pass()?;
    }
    driver.cast_at(answer, &[at])?;
    settle(&mut driver, Target::Player(pid(0)))?;
    let life = |seat: usize| driver.state().player(pid(seat)).unwrap().life();
    Ok((life(0), life(1)))
}

#[rstest]
#[case::two_seats(2)]
#[case::four_seats(4)]
fn invert_polarity_takes_the_spell_or_counters_it(#[case] seats: usize) -> Result<(), ScriptError> {
    let mut seen = Vec::new();
    for seed in 0..12 {
        let lives = bolt_answered(seats, seed, "Invert Polarity", None)?;
        assert!(
            [(17, 20), (20, 20)].contains(&lives),
            "seed {seed}: {lives:?}"
        );
        seen.push(lives);
    }
    assert!(
        seen.contains(&(17, 20)) && seen.contains(&(20, 20)),
        "both flips happen: {seen:?}"
    );
    Ok(())
}

#[rstest]
#[case::two_seats(2)]
#[case::three_seats(3)]
fn wylls_reversal_copies_only_on_fifteen_or_more(#[case] seats: usize) -> Result<(), ScriptError> {
    for seed in 0..4 {
        assert_eq!(
            bolt_answered(seats, seed, "Wyll's Reversal", Some(20))?,
            (14, 20),
            "seed {seed}"
        );
    }
    let mut seen = Vec::new();
    for seed in 0..12 {
        let lives = bolt_answered(seats, seed, "Wyll's Reversal", None)?;
        assert!(
            [(17, 20), (14, 20)].contains(&lives),
            "seed {seed}: {lives:?}"
        );
        seen.push(lives);
    }
    assert!(
        seen.contains(&(17, 20)),
        "a low roll only points the spell elsewhere: {seen:?}"
    );
    Ok(())
}

/// Seat 1 answers seat 0's Bolt with Disrupting Shoal for X, by exiling a blue card of mana value `exiled` when given,
/// else by paying its mana; seat 1's life once all resolved.
fn shoal(seats: usize, x: u32, exiled: Option<u32>) -> Result<i32, ScriptError> {
    let blue = instant(
        "Insight",
        exiled.unwrap_or(1).saturating_sub(1),
        &[Color::Blue],
    );
    let mut driver = Driver::from_scenario(
        Scenario::new(seats)
            .hand(0, [burn("Bolt", 3)])
            .battlefield(0, [ready(mountain())])
            .hand(1, [card("Disrupting Shoal").unwrap(), blue])
            .battlefield(1, (0..4).map(|_| ready(island()))),
    )?;
    driver.begin_cast("Bolt")?;
    driver.answer(Answer::ChooseTargets(TargetChoice::Targets(vec![
        Target::Player(pid(1)),
    ])))?;
    driver.answer(Answer::PayMana(PayChoice::Auto))?;
    let on_stack = driver.state().stack()[0];
    let at = Target::Object(driver.state().object(on_stack).unwrap().reference());
    while driver.decision()?.player != pid(1) {
        driver.pass()?;
    }
    driver.begin_cast("Disrupting Shoal")?;
    for _ in 0..60 {
        let decision = driver.decision()?.clone();
        if matches!(decision.kind, DecisionKind::Priority(_)) && driver.state().stack().is_empty() {
            break;
        }
        let answer = match &decision.kind {
            DecisionKind::Priority(_) => {
                driver.pass()?;
                continue;
            }
            DecisionKind::ChooseOption(_) => {
                let offered = ways(&driver)?;
                let free = offered
                    .iter()
                    .position(|way| way.to_lowercase().contains("exile"));
                let at = if exiled.is_some() {
                    free.unwrap()
                } else {
                    free.map_or(0, |free| usize::from(free == 0))
                };
                Answer::ChooseOption(OptionChoice(at))
            }
            DecisionKind::ChooseNumber(_) => Answer::ChooseNumber(NumberChoice::Number(x)),
            DecisionKind::ChooseTargets(_) => {
                Answer::ChooseTargets(TargetChoice::Targets(vec![at]))
            }
            DecisionKind::ChooseCards(o) => Answer::ChooseCards(CardsChoice(o.from[..1].to_vec())),
            DecisionKind::PayMana(_) => Answer::PayMana(PayChoice::Auto),
            _ => Driver::passive_answer(&decision),
        };
        driver.answer(answer)?;
    }
    Ok(driver.state().player(pid(1)).unwrap().life())
}

#[rstest]
#[case::paid_x_one(2, 1, None, 20)]
#[case::paid_x_two(3, 2, None, 17)]
#[case::exiled_one(2, 1, Some(1), 20)]
#[case::exiled_two(4, 2, Some(2), 17)]
fn disrupting_shoal_counters_only_a_spell_of_mana_value_x(
    #[case] seats: usize,
    #[case] x: u32,
    #[case] exiled: Option<u32>,
    #[case] life: i32,
) -> Result<(), ScriptError> {
    assert_eq!(shoal(seats, x, exiled)?, life);
    Ok(())
}
