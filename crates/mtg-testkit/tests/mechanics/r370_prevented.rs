//! Goal: a shield that prevents the next damage and then deals the damage it prevented to a target of its spell
//! (Acolyte's Reward, `ReplaceBy::PreventCountedAnd` with `Ref::Target`) deals exactly the amount it prevented to the
//! target chosen as the spell was cast, at any table size, and no more than its counted total.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_core::color::Color;
use mtg_core::mana::ManaSymbol;
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_testkit::cards::{burn, cost, creature_costing, free_creature};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::pid;

/// The corpus card with no mana cost, so it is cast without lands.
fn free(name: &str) -> Arc<CardDefinition> {
    Arc::new(
        card(name)
            .unwrap()
            .as_ref()
            .clone()
            .with_mana_cost(cost(0, &[])),
    )
}

/// Passes priority until the stack is empty, answering anything else passively.
fn settle(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..100 {
        let decision = driver.decision()?.clone();
        match decision.kind {
            DecisionKind::Priority(_)
                if driver.state().stack().is_empty() && driver.state().resolving().is_none() =>
            {
                return Ok(());
            }
            DecisionKind::Priority(_) => {
                driver.pass()?;
            }
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

#[rstest]
#[case::less_than_the_damage(2, 3)]
#[case::all_of_it(3, 3)]
#[case::more_than_the_damage(5, 3)]
fn r370_the_damage_prevented_is_dealt_to_the_target(
    #[values(2, 3, 4)] seats: usize,
    #[case] devotion: usize,
    #[case] bolt: i32,
) -> Result<(), ScriptError> {
    let shrine = creature_costing(
        "Shrine",
        &vec![ManaSymbol::Colored(Color::White); devotion],
        0,
        4,
    );
    let foe = seats - 1;
    let mut driver = Driver::from_scenario(
        Scenario::new(seats)
            .battlefield(0, [ready(free_creature("Bear", 2, 2)), ready(shrine)])
            .hand(
                0,
                [
                    free("Acolyte's Reward"),
                    Arc::new(
                        burn("Bolt", bolt)
                            .as_ref()
                            .clone()
                            .with_mana_cost(cost(0, &[])),
                    ),
                ],
            ),
    )?;
    let reward = driver.state().player(pid(foe)).unwrap().life();
    driver.begin_cast("Acolyte's Reward")?;
    let target = driver.target_named("Bear")?;
    driver.choose_targets(&[target])?;
    let player = driver.target_player(foe);
    driver.choose_targets(&[player])?;
    driver.answer(Answer::PayMana(PayChoice::Auto))?;
    settle(&mut driver)?;
    let target = driver.target_named("Bear")?;
    driver.begin_cast("Bolt")?;
    driver.choose_targets(&[target])?;
    driver.answer(Answer::PayMana(PayChoice::Auto))?;
    settle(&mut driver)?;
    let prevented = devotion.min(usize::try_from(bolt).unwrap());
    let prevented_i32 = i32::try_from(prevented).unwrap();
    assert_eq!(
        driver.state().player(pid(foe)).unwrap().life(),
        reward - prevented_i32,
        "the foe takes what was prevented"
    );
    let bear = driver.id_of("Bear")?;
    let marked = driver
        .state()
        .object(bear)
        .unwrap()
        .permanent()
        .unwrap()
        .damage();
    assert_eq!(i64::from(marked), i64::from(bolt - prevented_i32));
    for seat in 1..foe {
        assert_eq!(driver.state().player(pid(seat)).unwrap().life(), reward);
    }
    Ok(())
}
