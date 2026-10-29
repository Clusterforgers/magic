//! Goal: the control and payment sentences the compiler reads in the t12c sweep (Word of Seizing, Homeward Path after
//! a permanent theft, Diffusion Sliver) play through the engine at tables of two, three and four seats: an untapped
//! permanent taken for the turn goes home with the turn, a player who owns what another controls takes it back for good,
//! a targeting spell is countered when its controller cannot pay; every answer keeps the invariants and the log replays
//! to the state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::VecDeque;
use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_engine::choose::{Target, TargetChoice};
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::turn::Step;
use mtg_testkit::cards::{free_creature, island, mountain};
use mtg_testkit::{Driver, Scenario, ScriptError, ready, tapped};
use rstest::rstest;

use crate::common::copying::replays;
use crate::w7d_control_scenarios::w7d_oracle;

type W20cResult = Result<(), ScriptError>;

/// Word of Seizing as printed.
fn w20c_seizing() -> Arc<CardDefinition> {
    w7d_oracle(
        "Word of Seizing",
        "{3}{R}",
        "Instant",
        None,
        "Split second (As long as this spell is on the stack, players can't cast spells or activate abilities that aren't mana abilities.)\nUntap target permanent and gain control of it until end of turn. It gains haste until end of turn.",
    )
}

/// Homeward Path as printed.
fn w20c_path() -> Arc<CardDefinition> {
    w7d_oracle(
        "Homeward Path",
        "",
        "Land",
        None,
        "{T}: Add {C}.\n{T}: Each player gains control of all creatures they own.",
    )
}

/// A permanent theft, so a creature has an owner other than its controller.
fn w20c_theft() -> Arc<CardDefinition> {
    w7d_oracle(
        "Blatant Thievery",
        "{U}",
        "Sorcery",
        None,
        "For each opponent, gain control of target permanent that player controls.",
    )
}

/// Diffusion Sliver as printed.
fn w20c_sliver() -> Arc<CardDefinition> {
    w7d_oracle(
        "Diffusion Sliver",
        "{2}{U}",
        "Creature \u{2014} Sliver",
        Some((2, 2)),
        "Whenever a Sliver creature you control becomes the target of a spell or ability an opponent controls, counter that spell or ability unless its controller pays {2}.",
    )
}

/// A burn spell with no other effect.
fn w20c_bolt() -> Arc<CardDefinition> {
    w7d_oracle(
        "Bolt",
        "{R}",
        "Instant",
        None,
        "Bolt deals 3 damage to target creature.",
    )
}

/// Answers until the stack is empty, nothing resolves and no trigger waits: each target decision takes the next of
/// `picks`, mana is paid automatically, anything else is answered passively.
fn w20c_answer(driver: &mut Driver, picks: &mut VecDeque<Target>) -> W20cResult {
    for _ in 0..300 {
        let decision = driver.decision()?.clone();
        let idle = driver.state().stack().is_empty()
            && driver.state().resolving().is_none()
            && driver.state().pending_triggers().is_empty();
        if matches!(decision.kind, DecisionKind::Priority(_)) && idle {
            return Ok(());
        }
        let answer = match &decision.kind {
            DecisionKind::ChooseTargets(options) if !picks.is_empty() => {
                let take = (options.max as usize).min(picks.len());
                Answer::ChooseTargets(TargetChoice::Targets(picks.drain(..take).collect()))
            }
            DecisionKind::PayMana(_) => Answer::PayMana(PayChoice::Auto),
            _ => Driver::passive_answer(&decision),
        };
        driver.answer(answer)?;
    }
    Err(ScriptError::Stuck("the stack".into()))
}

/// The deciding player casts `name` at `picks` and every seat answers until the stack is empty.
fn w20c_cast(driver: &mut Driver, name: &str, picks: Vec<Target>) -> W20cResult {
    driver.begin_cast(name)?;
    w20c_answer(driver, &mut picks.into())
}

/// The seat that controls the permanent called `name`.
fn w20c_holds(driver: &Driver, name: &str) -> usize {
    let id = driver.id_of(name).unwrap();
    mtg_engine::characteristics::controller_of(driver.state(), id)
        .unwrap()
        .index()
}

#[rstest]
fn w20c_a_tapped_permanent_is_taken_untapped_for_the_turn(
    #[values(2, 3, 4)] seats: usize,
) -> W20cResult {
    let lands: Vec<_> = (0..4).map(|_| ready(mountain())).collect();
    let scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .hand(0, [w20c_seizing()])
        .battlefield(0, lands)
        .battlefield(1, [tapped(free_creature("Ogre", 3, 3))]);
    let setup = scenario.clone().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario)?;
    let ogre = driver.target_named("Ogre")?;
    w20c_cast(&mut driver, "Word of Seizing", vec![ogre])?;
    assert_eq!(w20c_holds(&driver, "Ogre"), 0, "taken for the turn");
    let id = driver.id_of("Ogre")?;
    let held = driver.state().object(id).unwrap().permanent().unwrap();
    assert!(!held.tapped(), "untapped by the spell");
    driver.idle_until(2, Step::Main1)?;
    assert_eq!(w20c_holds(&driver, "Ogre"), 1, "home with the turn");
    replays(&driver, &setup);
    Ok(())
}

#[rstest]
fn w20c_an_owner_takes_back_what_another_player_controls(
    #[values(2, 3, 4)] seats: usize,
) -> W20cResult {
    let scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .hand(0, [w20c_theft()])
        .battlefield(0, [ready(island()), ready(w20c_path())])
        .battlefield(1, [ready(free_creature("Ogre", 3, 3))]);
    let setup = scenario.clone().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario)?;
    let ogre = driver.target_named("Ogre")?;
    w20c_cast(&mut driver, "Blatant Thievery", vec![ogre])?;
    assert_eq!(w20c_holds(&driver, "Ogre"), 0, "stolen for good");
    driver.begin_ability("Homeward Path", 0)?;
    w20c_answer(&mut driver, &mut VecDeque::new())?;
    assert_eq!(w20c_holds(&driver, "Ogre"), 1, "back with its owner");
    replays(&driver, &setup);
    Ok(())
}

#[rstest]
fn w20c_a_targeting_spell_is_countered_when_it_is_not_paid_for(
    #[values(2, 3, 4)] seats: usize,
) -> W20cResult {
    let scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .hand(0, [w20c_bolt()])
        .battlefield(0, [ready(mountain())])
        .battlefield(1, [ready(w20c_sliver())]);
    let setup = scenario.clone().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario)?;
    let sliver = driver.target_named("Diffusion Sliver")?;
    w20c_cast(&mut driver, "Bolt", vec![sliver])?;
    let id = driver.id_of("Diffusion Sliver")?;
    let sliver = driver.state().object(id).unwrap().permanent().unwrap();
    assert_eq!(sliver.damage(), 0, "the spell was countered, not resolved");
    replays(&driver, &setup);
    Ok(())
}
