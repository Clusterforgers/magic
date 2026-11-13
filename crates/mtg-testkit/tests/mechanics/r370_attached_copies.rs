//! Goal: a copy effect that lasts for as long as an Equipment stays attached (`Duration::WhileSourceAttached`, Blade of
//! Shared Souls) makes the equipped creature a copy only when its controller says yes, and ends once the Equipment
//! moves to another creature, at any table size.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_engine::choose::YesNoChoice;
use mtg_engine::decision::{Answer, DecisionKind, PayChoice, PriorityAction};
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{cost, free_creature, plains};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

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

/// Passes priority until the stack is empty: a target is the creature named `target`, a "may" is `may`, anything else
/// is answered passively.
fn settle(driver: &mut Driver, target: &str, may: bool) -> Result<(), ScriptError> {
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
            DecisionKind::ChooseTargets(_) => {
                let chosen = driver.target_named(target)?;
                driver.choose_targets(&[chosen])?;
            }
            DecisionKind::YesNo(_) => {
                driver.answer(Answer::YesNo(YesNoChoice(may)))?;
            }
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    Err(ScriptError::Stuck("the stack".into()))
}

/// The names of the permanents on the battlefield, as they are now, sorted.
fn names(driver: &Driver) -> Vec<String> {
    let state = driver.state();
    let mut names: Vec<String> = state
        .zone(Zone::Battlefield)
        .iter()
        .filter_map(|id| state.object(*id))
        .map(|object| object.card().name.clone())
        .filter(|name| name != "Plains")
        .collect();
    names.sort();
    names
}

#[rstest]
#[case::copies(true, &["Bear", "Blade of Shared Souls", "Ogre", "Ogre"])]
#[case::declines(false, &["Bear", "Blade of Shared Souls", "Ogre", "Rebel"])]
fn r370_the_copy_lasts_while_attached(
    #[values(2, 3, 4)] seats: usize,
    #[case] may: bool,
    #[case] while_attached: &[&str],
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(seats)
            .battlefield(
                0,
                [
                    ready(free_creature("Ogre", 3, 3)),
                    ready(free_creature("Bear", 2, 2)),
                    ready(plains()),
                    ready(plains()),
                ],
            )
            .hand(0, [free("Blade of Shared Souls")]),
    )?;
    driver.cast("Blade of Shared Souls")?;
    settle(&mut driver, "Ogre", may)?;
    assert_eq!(names(&driver), while_attached);
    // Equip moves the Blade to the Bear; the token is a Rebel again, and the Bear stays a Bear.
    let blade = driver.id_of("Blade of Shared Souls")?;
    let DecisionKind::Priority(options) = &driver.decision()?.kind else {
        panic!("priority was expected");
    };
    let equip = options
        .abilities
        .iter()
        .find(|ability| ability.object == blade)
        .copied()
        .unwrap();
    driver.answer(Answer::Priority(PriorityAction::Activate(equip)))?;
    let bear = driver.target_named("Bear")?;
    driver.choose_targets(&[bear])?;
    settle(&mut driver, "Ogre", false)?;
    assert_eq!(
        names(&driver),
        ["Bear", "Blade of Shared Souls", "Ogre", "Rebel"]
    );
    Ok(())
}
