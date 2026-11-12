//! Goal: a shield that prevents the next N damage and then acts on the damage it prevented (Test of Faith's counters,
//! Candles' Glow's life) prevents no more than N in all, over several damage events, acts once for each event with the
//! amount prevented from it, and lets the rest of the damage through, at tables of two, three and four seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use common::casting::lands;
use common::{pid, spell};
use mtg_cards::value::CounterKind;
use mtg_core::card::CardType;
use mtg_engine::choose::NumberChoice;
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::turn::Step;
use mtg_testkit::cards::{free_creature, plains};
use mtg_testkit::corpus::corpus;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// Casts `name` at `targets`, answering X with `x`, paying automatically, and lets every spell resolve.
fn cast_resolved(
    driver: &mut Driver,
    name: &str,
    targets: &[mtg_engine::choose::Target],
    x: u32,
) -> Result<(), ScriptError> {
    driver.begin_cast(name)?;
    let mut paid = false;
    for _ in 0..80 {
        if paid && driver.state().stack().is_empty() && driver.state().resolving().is_none() {
            return Ok(());
        }
        match &driver.decision()?.kind {
            DecisionKind::ChooseNumber(_) => {
                driver.answer(Answer::ChooseNumber(NumberChoice::Number(x)))?;
            }
            DecisionKind::ChooseTargets(_) => {
                driver.choose_targets(targets)?;
            }
            DecisionKind::PayMana(_) => {
                paid = true;
                driver.answer(Answer::PayMana(PayChoice::Auto))?;
            }
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    Err(ScriptError::Stuck(format!("casting {name}")))
}

/// A burn spell of seat 0 that deals `amount` damage to any target.
fn burn(name: &str, amount: u32) -> std::sync::Arc<mtg_cards::CardDefinition> {
    spell(
        name,
        CardType::Instant,
        r#"[{"what":"AnyTarget"}]"#,
        &format!(r#"[{{"DealDamage":{{"to":{{"Target":0}},"amount":{amount}}}}}]"#),
    )
}

#[rstest]
#[case::faith_takes_all_of_a_small_hit("Test of Faith", 2, 0, (0, 2))]
#[case::faith_stops_at_three("Test of Faith", 5, 0, (2, 3))]
#[case::faith_over_two_hits("Test of Faith", 2, 2, (1, 3))]
#[case::glow_gains_what_it_prevents("Candles' Glow", 5, 0, (2, 3))]
#[case::glow_over_two_hits("Candles' Glow", 2, 2, (1, 3))]
fn r283_the_shield_prevents_up_to_its_total_and_acts_on_each_part(
    #[case] card: &str,
    #[case] first: u32,
    #[case] second: u32,
    #[case] (damage, gained): (u32, u32),
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(seats)
            .at(1, 0, Step::Main1)
            .hand(
                0,
                [
                    corpus().unwrap().card(card).unwrap(),
                    burn("First", first),
                    burn("Second", second),
                ],
            )
            .battlefield(0, {
                let mut permanents = lands(10, plains);
                permanents.push(ready(free_creature("Bear", 2, 9)));
                permanents
            }),
    )?;
    let bear = driver.target_named("Bear")?;
    cast_resolved(&mut driver, card, &[bear], 0)?;
    cast_resolved(&mut driver, "First", &[bear], 0)?;
    if second > 0 {
        cast_resolved(&mut driver, "Second", &[bear], 0)?;
    }
    let state = driver.state();
    let id = driver.id_of("Bear")?;
    let object = state.object(id).unwrap();
    assert_eq!(object.permanent().unwrap().damage(), damage);
    if card == "Test of Faith" {
        assert_eq!(
            object
                .permanent()
                .unwrap()
                .counters_of(&CounterKind::PlusOnePlusOne),
            gained
        );
    } else {
        assert_eq!(
            state.player(pid(0)).unwrap().life(),
            20 + i32::try_from(gained).unwrap()
        );
    }
    Ok(())
}
