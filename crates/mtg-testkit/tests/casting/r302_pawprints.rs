//! Goal: modes counted in pawprints ("Choose up to five {P} worth of modes. You may choose the same mode more than once.")
//! are offered with their pawprints, a choice whose modes take at most five pawprints is accepted and resolves each mode
//! as often as it was chosen, and a choice that takes more is refused, at tables of two to four seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_core::card::CardType;
use mtg_engine::choose::ModeChoice;
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::turn::Step;
use mtg_testkit::{Driver, Scenario, ScriptError};
use rstest::rstest;

use crate::common::{pid, spell};

/// A season: {P} gains 1 life, {P}{P} gains 10 and {P}{P}{P} gains 100.
fn season() -> Arc<CardDefinition> {
    spell(
        "Season",
        CardType::Sorcery,
        "[]",
        r#"[{"Modal":{"choose":{"min":0,"max":5},"repeat":true,"pawprints":[1,2,3],"modes":[
            {"effects":[{"GainLife":{"amount":1}}]},
            {"effects":[{"GainLife":{"amount":10}}]},
            {"effects":[{"GainLife":{"amount":100}}]}]}}]"#,
    )
}

#[rstest]
#[case::five_single_pawprints(&[0, 0, 0, 0, 0], Some(5))]
#[case::two_and_three(&[2, 1], Some(110))]
#[case::one_one_and_two(&[1, 0, 0, 0], Some(13))]
#[case::none(&[], Some(0))]
#[case::six_pawprints(&[1, 1, 0, 0], None)]
#[case::three_and_three(&[2, 2], None)]
fn modes_take_their_pawprints_from_the_most(
    #[values(2, 3, 4)] seats: usize,
    #[case] modes: &[usize],
    #[case] gained: Option<i32>,
) -> Result<(), ScriptError> {
    let scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .hand(0, vec![season()]);
    let mut driver = Driver::from_scenario(scenario)?;
    let before = driver.state().player(pid(0)).unwrap().life();
    driver.begin_cast("Season")?;
    let DecisionKind::ChooseMode(options) = &driver.decision()?.kind else {
        panic!("the modes are asked first");
    };
    let weights: Vec<Option<u32>> = options.modes.iter().map(|mode| mode.weight).collect();
    assert_eq!(weights, vec![Some(1), Some(2), Some(3)]);
    assert_eq!(options.max, 5);
    let answered = driver.answer(Answer::ChooseMode(ModeChoice::Modes(modes.to_vec())));
    let Some(gained) = gained else {
        assert!(
            answered.is_err(),
            "{modes:?} takes more than five pawprints"
        );
        return Ok(());
    };
    answered?;
    for _ in 0..60 {
        let asked = matches!(driver.decision()?.kind, DecisionKind::Priority(_));
        if asked && driver.state().stack().is_empty() && driver.state().resolving().is_none() {
            break;
        }
        if matches!(driver.decision()?.kind, DecisionKind::PayMana(_)) {
            driver.answer(Answer::PayMana(PayChoice::Auto))?;
        } else {
            driver.answer_passively()?;
        }
    }
    let after = driver.state().player(pid(0)).unwrap().life();
    assert_eq!(i32::try_from(after - before).unwrap(), gained, "{modes:?}");
    Ok(())
}
