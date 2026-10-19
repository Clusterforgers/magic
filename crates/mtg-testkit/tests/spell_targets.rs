//! Goal: a spell with a target is announced, paid for, put on the stack and resolved through the effect pipeline: Lightning Bolt from the corpus kills a creature and hurts a player, a cast can be backed out of for free, and a spell whose targets have all become illegal does nothing at all.
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use common::names;
use mtg_engine::Event;
use mtg_engine::choose::Target;
use mtg_engine::decision::DecisionKind;
use mtg_engine::events::ResolutionOutcome;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{
    burn, burn_own_creature, drain, forest, free_creature, mountain, salve, swamp, twin_burn,
};
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

/// Seat 0 holds `salve` (2 damage to a target, gain 2 life, draw a card) and seat 1 holds a Bolt.
fn salve_scenario() -> Scenario {
    Scenario::new(2)
        .hand(0, [salve("Salve")])
        .battlefield(0, [ready(forest()), ready(forest())])
        .hand(1, [burn("Bolt", 3)])
        .battlefield(1, [ready(free_creature("Bear", 2, 2)), ready(mountain())])
}

#[test]
fn a_spell_that_resolves_does_all_of_what_it_says() -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(salve_scenario())?;
    let hand = driver.state().zone(Zone::Hand(common::pid(0))).len();
    let at = driver.target_named("Bear")?;
    driver.cast_at("Salve", &[at])?;
    resolve_the_stack(&mut driver)?;
    driver.expect_life(0, 22)?;
    assert_eq!(
        driver.state().zone(Zone::Hand(common::pid(0))).len(),
        hand,
        "the Salve left the hand and a card was drawn"
    );
    driver.expect_zone(Zone::Graveyard(common::pid(1)), &["Bear"])?;
    Ok(())
}

#[test]
fn a_spell_whose_target_left_does_nothing_at_all_not_even_the_parts_that_do_not_use_it()
-> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(salve_scenario())?;
    let hand = driver.state().zone(Zone::Hand(common::pid(0))).len();
    let at = driver.target_named("Bear")?;
    driver.cast_at("Salve", &[at])?;
    driver.pass()?;
    let at = driver.target_named("Bear")?;
    driver.cast_at("Bolt", &[at])?;
    let events = resolve_the_stack(&mut driver)?;
    // CR 608.2b: no life gained and no card drawn, though neither step names the target.
    driver.expect_life(0, 20)?;
    assert_eq!(
        driver.state().zone(Zone::Hand(common::pid(0))).len(),
        hand - 1,
        "only the Salve left the hand"
    );
    assert!(
        !events
            .iter()
            .any(|e| matches!(e, Event::LifeGained { .. } | Event::CardDrawn { .. }))
    );
    driver.expect_zone(Zone::Graveyard(common::pid(0)), &["Salve"])?;
    Ok(())
}

#[test]
fn a_spell_with_two_targets_keeps_its_legal_one_and_fizzles_only_when_both_are_gone()
-> Result<(), ScriptError> {
    let scenario = || {
        Scenario::new(2)
            .hand(0, [twin_burn("Twin Burn")])
            .battlefield(0, [ready(mountain()), ready(mountain()), ready(mountain())])
            .hand(1, [burn("Bolt", 3), burn("Second Bolt", 3)])
            .battlefield(
                1,
                [
                    ready(free_creature("Bear", 2, 2)),
                    ready(free_creature("Wolf", 2, 2)),
                    ready(mountain()),
                    ready(mountain()),
                ],
            )
    };
    // Announcement: one decision for each specification, in order, each free to back out of.
    let mut driver = Driver::from_scenario(scenario())?;
    driver.begin_cast("Twin Burn")?;
    let specs: Vec<usize> = (0..2)
        .map(|_| {
            let DecisionKind::ChooseTargets(options) = &driver.decision().unwrap().kind else {
                panic!("a target decision");
            };
            let spec = options.spec;
            let first = options.legal[0];
            driver.choose_targets(&[first]).unwrap();
            spec
        })
        .collect();
    assert_eq!(specs, [0, 1]);
    driver.expect_decision(0, "pay-mana")?;

    // One of the two targets dies in response: the spell resolves and hurts the other.
    let mut driver = Driver::from_scenario(scenario())?;
    let (bear, wolf) = (driver.target_named("Bear")?, driver.target_named("Wolf")?);
    driver.begin_cast("Twin Burn")?;
    driver.choose_targets(&[bear])?;
    driver.choose_targets(&[wolf])?;
    driver.answer(mtg_engine::Answer::PayMana(
        mtg_engine::decision::PayChoice::Auto,
    ))?;
    driver.pass()?;
    let at = driver.target_named("Bear")?;
    driver.cast_at("Bolt", &[at])?;
    let events = resolve_the_stack(&mut driver)?;
    driver.expect_zone(Zone::Graveyard(common::pid(1)), &["Bolt", "Bear", "Wolf"])?;
    let outcomes: Vec<ResolutionOutcome> = events
        .iter()
        .filter_map(|e| match e {
            Event::ResolutionEnded { outcome, .. } => Some(*outcome),
            _ => None,
        })
        .collect();
    assert_eq!(
        outcomes,
        [ResolutionOutcome::Resolved, ResolutionOutcome::Resolved]
    );

    // Both die in response: it fizzles.
    let mut driver = Driver::from_scenario(scenario())?;
    let (bear, wolf) = (driver.target_named("Bear")?, driver.target_named("Wolf")?);
    driver.begin_cast("Twin Burn")?;
    driver.choose_targets(&[bear])?;
    driver.choose_targets(&[wolf])?;
    driver.answer(mtg_engine::Answer::PayMana(
        mtg_engine::decision::PayChoice::Auto,
    ))?;
    driver.pass()?;
    let at = driver.target_named("Bear")?;
    driver.cast_at("Bolt", &[at])?;
    let at = driver.target_named("Wolf")?;
    driver.cast_at("Second Bolt", &[at])?;
    let events = resolve_the_stack(&mut driver)?;
    let outcomes: Vec<ResolutionOutcome> = events
        .iter()
        .filter_map(|e| match e {
            Event::ResolutionEnded { outcome, .. } => Some(*outcome),
            _ => None,
        })
        .collect();
    assert_eq!(
        outcomes,
        [
            ResolutionOutcome::Resolved,
            ResolutionOutcome::Resolved,
            ResolutionOutcome::Fizzled
        ]
    );
    Ok(())
}

#[test]
fn a_spell_is_castable_only_when_it_has_a_legal_target_for_each_specification()
-> Result<(), ScriptError> {
    let without = Driver::from_scenario(
        Scenario::new(2)
            .hand(0, [burn_own_creature("Rebuke")])
            .battlefield(0, [ready(mountain())])
            .battlefield(1, [ready(free_creature("Bear", 2, 2))]),
    )?;
    assert!(
        castable(&without).is_empty(),
        "the only creature is the opponent's"
    );
    let with = Driver::from_scenario(
        Scenario::new(2)
            .hand(0, [burn_own_creature("Rebuke")])
            .battlefield(0, [ready(mountain()), ready(free_creature("Mine", 2, 2))]),
    )?;
    assert_eq!(castable(&with).len(), 1);
    Ok(())
}

fn castable(driver: &Driver) -> Vec<mtg_core::ids::ObjectId> {
    match &driver.decision().unwrap().kind {
        DecisionKind::Priority(options) => options.castable.clone(),
        other => panic!("priority was expected, got {}", other.name()),
    }
}

#[test]
fn a_seat_holding_an_instant_can_answer_what_is_on_the_stack_and_one_holding_a_sorcery_cannot()
-> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(2)
            .hand(0, [burn("Bolt", 3)])
            .battlefield(0, [ready(mountain())])
            .hand(1, [burn("Counter-Bolt", 3), drain("Siphon")])
            .battlefield(1, [ready(mountain()), ready(swamp()), ready(swamp())]),
    )?;
    let at = driver.target_player(1);
    driver.cast_at("Bolt", &[at])?;
    driver.pass()?;
    let DecisionKind::Priority(options) = &driver.decision()?.kind else {
        panic!("seat 1 has priority");
    };
    assert_eq!(driver.decision()?.player, common::pid(1));
    // The instant can be cast over the stack; the sorcery, which could be cast if the stack were empty, cannot.
    let names: Vec<String> = options
        .castable
        .iter()
        .map(|id| driver.state().object(*id).unwrap().card().name.clone())
        .collect();
    assert_eq!(names, ["Counter-Bolt"]);
    assert!(options.can_act);
    Ok(())
}

#[test]
fn a_sorcery_is_cast_in_your_own_main_phase_when_the_stack_is_empty() -> Result<(), ScriptError> {
    let driver = Driver::from_scenario(
        Scenario::new(3)
            .hand(0, [drain("Siphon")])
            .battlefield(0, [ready(swamp()), ready(swamp())]),
    )?;
    assert_eq!(castable(&driver).len(), 1);
    let mut driver = driver;
    driver.begin_cast("Siphon")?;
    let DecisionKind::ChooseTargets(options) = &driver.decision()?.kind else {
        panic!("a target");
    };
    // Target opponent: both of the other seats, and not the caster.
    assert_eq!(
        options.legal,
        [driver.target_player(1), driver.target_player(2)]
    );
    let at = driver.target_player(2);
    driver.choose_targets(&[at])?;
    driver.answer(mtg_engine::Answer::PayMana(
        mtg_engine::decision::PayChoice::Auto,
    ))?;
    resolve_the_stack(&mut driver)?;
    driver.expect_life(2, 18)?;
    driver.expect_life(0, 22)?;
    driver.expect_life(1, 20)?;
    Ok(())
}
