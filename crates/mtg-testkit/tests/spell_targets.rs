//! Goal: a spell with a target is announced, paid for, put on the stack and resolved through the effect pipeline: Lightning Bolt from the corpus kills a creature and hurts a player, a cast can be backed out of for free, and a spell whose targets have all become illegal does nothing at all.
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use common::names;
use mtg_engine::Event;
use mtg_engine::choose::Target;
use mtg_engine::decision::DecisionKind;
use mtg_engine::events::ResolutionOutcome;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{free_creature, mountain};
use mtg_testkit::{Driver, Scenario, ScriptError, ready};

fn bolt_scenario() -> Scenario {
    Scenario::new(2)
        .hand_named(0, ["Lightning Bolt"])
        .battlefield(0, [ready(mountain())])
        .battlefield(1, [ready(free_creature("Bear", 2, 2))])
}

fn resolve_the_stack(driver: &mut Driver) -> Result<Vec<Event>, ScriptError> {
    let mut events = Vec::new();
    while !driver.state().stack().is_empty() {
        events.extend(driver.pass()?);
    }
    Ok(events)
}

#[test]
fn a_bolt_at_a_player_takes_three_life() -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(bolt_scenario())?;
    let at = driver.target_player(1);
    driver.cast_at("Lightning Bolt", &[at])?;
    let events = resolve_the_stack(&mut driver)?;
    driver.expect_life(1, 17)?;
    driver.expect_zone(Zone::Graveyard(common::pid(0)), &["Lightning Bolt"])?;
    assert!(driver.state().resolving().is_none());
    let kinds: Vec<&str> = events
        .iter()
        .map(|e| match e {
            Event::ResolutionBegan { .. } => "began",
            Event::DamageDealt { .. } => "damage",
            Event::StepDone { .. } => "step",
            Event::ResolutionEnded { .. } => "ended",
            Event::SpellResolved { .. } => "resolved",
            _ => "other",
        })
        .filter(|kind| *kind != "other")
        .collect();
    assert_eq!(kinds, ["began", "damage", "step", "ended", "resolved"]);
    Ok(())
}

#[test]
fn a_bolt_at_a_creature_destroys_it_by_the_usual_state_based_action() -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(bolt_scenario())?;
    let at = driver.target_named("Bear")?;
    driver.cast_at("Lightning Bolt", &[at])?;
    resolve_the_stack(&mut driver)?;
    assert_eq!(names(driver.state(), Zone::Battlefield), ["Mountain"]);
    driver.expect_zone(Zone::Graveyard(common::pid(1)), &["Bear"])?;
    driver.expect_life(1, 20)?;
    Ok(())
}

#[test]
fn the_cast_announces_its_target_before_the_payment_and_logs_it_with_the_spell()
-> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(bolt_scenario())?;
    driver.begin_cast("Lightning Bolt")?;
    let DecisionKind::ChooseTargets(options) = &driver.decision()?.kind else {
        panic!("the target comes first");
    };
    assert_eq!(
        (options.spec, options.min, options.max, options.can_cancel),
        (0, 1, 1, true)
    );
    // Any target: both players and the creature, players first.
    assert_eq!(options.legal.len(), 3);
    let at = driver.target_named("Bear")?;
    driver.choose_targets(&[at])?;
    driver.expect_decision(0, "pay-mana")?;
    let events = driver.answer(mtg_engine::Answer::PayMana(
        mtg_engine::decision::PayChoice::Auto,
    ))?;
    let Some(Event::SpellCast { choices, .. }) =
        events.iter().find(|e| matches!(e, Event::SpellCast { .. }))
    else {
        panic!("the spell was cast");
    };
    assert_eq!(choices.targets, [vec![at]]);
    Ok(())
}

#[test]
fn backing_out_at_the_target_or_the_payment_logs_nothing() -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(bolt_scenario())?;
    let before = (driver.engine().events().len(), driver.engine().state_hash());
    driver.begin_cast("Lightning Bolt")?;
    driver.cancel_targets()?;
    assert_eq!(
        (driver.engine().events().len(), driver.engine().state_hash()),
        before
    );
    driver.expect_decision(0, "priority")?;

    driver.begin_cast("Lightning Bolt")?;
    let at = driver.target_player(1);
    driver.choose_targets(&[at])?;
    driver.answer(mtg_engine::Answer::PayMana(
        mtg_engine::decision::PayChoice::Cancel,
    ))?;
    assert_eq!(
        (driver.engine().events().len(), driver.engine().state_hash()),
        before
    );
    driver.expect_decision(0, "priority")?;
    Ok(())
}

#[test]
fn a_spell_whose_only_target_left_does_nothing_and_goes_to_the_graveyard() -> Result<(), ScriptError>
{
    let mut driver = Driver::from_scenario(
        bolt_scenario()
            .hand_named(1, ["Lightning Bolt"])
            .battlefield(1, [ready(free_creature("Bear", 2, 2)), ready(mountain())]),
    )?;
    let bear = driver.target_named("Bear")?;
    driver.cast_at("Lightning Bolt", &[bear])?;
    // The opponent answers with their own Bolt at the creature, which dies first.
    driver.pass()?;
    driver.expect_decision(1, "priority")?;
    let at = driver.target_named("Bear")?;
    driver.cast_at("Lightning Bolt", &[at])?;
    let events = resolve_the_stack(&mut driver)?;
    let ended: Vec<ResolutionOutcome> = events
        .iter()
        .filter_map(|e| match e {
            Event::ResolutionEnded { outcome, .. } => Some(*outcome),
            _ => None,
        })
        .collect();
    assert_eq!(
        ended,
        [ResolutionOutcome::Resolved, ResolutionOutcome::Fizzled]
    );
    // The first Bolt dealt no damage to anyone: the Bear was already gone.
    let damage = events
        .iter()
        .filter(|e| matches!(e, Event::DamageDealt { .. }))
        .count();
    assert_eq!(damage, 1);
    driver.expect_zone(Zone::Graveyard(common::pid(0)), &["Lightning Bolt"])?;
    driver.expect_zone(Zone::Graveyard(common::pid(1)), &["Lightning Bolt", "Bear"])?;
    Ok(())
}

#[test]
fn a_target_is_an_object_until_it_changes_zones() {
    let driver = Driver::from_scenario(bolt_scenario()).unwrap();
    let Target::Object(bear) = driver.target_named("Bear").unwrap() else {
        panic!("an object");
    };
    assert_eq!(bear.counter, 0);
}
