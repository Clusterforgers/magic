//! Goal: Brass Infiniscope's mana ability adds {C}{C} without the stack and sets up "when you next cast a spell with {X}
//! in its mana cost this turn": a spell without {X} is passed over, the first spell with {X} triggers it once (a later
//! one does not), and it draws a card and gains half that spell's X rounded down, for every X and table size.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_core::card::CardType;
use mtg_engine::choose::NumberChoice;
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{free_creature, wastes};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::pid;

/// An artifact spell "{X}" with no rules.
fn x_relic(name: &str) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Artifact)
            .with_mana_cost(mtg_testkit::cards::cost(0, &[]))
            .with_x_in_cost(1),
    )
}

fn settle(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..200 {
        if driver.state().stack().is_empty()
            && driver.state().resolving().is_none()
            && driver.state().pending_triggers().is_empty()
        {
            return Ok(());
        }
        driver.answer_passively()?;
    }
    Err(ScriptError::Stuck("the stack".into()))
}

/// Casts `name` announcing `x` when asked, paying automatically.
fn cast_x(driver: &mut Driver, name: &str, x: u32) -> Result<(), ScriptError> {
    driver.begin_cast(name)?;
    if matches!(driver.decision()?.kind, DecisionKind::ChooseNumber(_)) {
        driver.answer(Answer::ChooseNumber(NumberChoice::Number(x)))?;
    }
    if matches!(driver.decision()?.kind, DecisionKind::PayMana(_)) {
        driver.answer(Answer::PayMana(mtg_engine::decision::PayChoice::Auto))?;
    }
    settle(driver)
}

#[rstest]
fn the_next_x_spell_draws_and_gains_half_x(
    #[values(0, 1, 2, 5)] x: u32,
    #[values(2, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let lands: Vec<_> = (0..8).map(|_| ready(wastes())).collect();
    let library: Vec<_> = (0..5).map(|_| wastes()).collect();
    let mut scenario = Scenario::new(seats)
        .battlefield(0, std::iter::once(ready(card("Brass Infiniscope").unwrap())).chain(lands))
        .hand(0, [free_creature("Plain Bear", 2, 2), x_relic("First X"), x_relic("Second X")])
        .library(0, library);
    for seat in 1..seats {
        scenario = scenario.battlefield(seat, [ready(free_creature("Bystander", 1, 1))]);
    }
    let mut driver = Driver::from_scenario(scenario)?;
    driver.tap_for_mana("Brass Infiniscope")?;
    assert!(driver.state().stack().is_empty(), "a mana ability uses no stack");
    let hand = |driver: &Driver| driver.state().zone(Zone::Hand(pid(0))).len();
    let life = |driver: &Driver| driver.state().player(pid(0)).unwrap().life();
    driver.cast("Plain Bear")?;
    settle(&mut driver)?;
    assert_eq!((hand(&driver), life(&driver)), (2, 20), "a spell without {{X}} is passed over");
    cast_x(&mut driver, "First X", x)?;
    let gained = i32::try_from(x / 2).unwrap();
    assert_eq!((hand(&driver), life(&driver)), (2, 20 + gained), "one card drawn, half X gained");
    cast_x(&mut driver, "Second X", x)?;
    assert_eq!((hand(&driver), life(&driver)), (1, 20 + gained), "it triggers once");
    Ok(())
}
