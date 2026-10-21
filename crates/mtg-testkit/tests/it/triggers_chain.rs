//! Goal: state-based actions run before every sweep of triggers and the sweep repeats until none waits, so a chain of deaths and the triggers they cause goes on the stack without a player receiving priority in between; a trigger that triggers while others are being put on the stack waits for the next round (CR 603.3b); and the triggers of a player who leaves the game while they wait are dropped.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use common::pid;
use mtg_cards::refs::{Pred, Ref, Selector, Targetable};
use mtg_core::card::CardType;
use mtg_engine::decision::DecisionKind;
use mtg_engine::turn::Step;
use mtg_engine::{Event, GameState};
use mtg_testkit::cards::{burn, damage, free_creature, gain_life, mountain};
use mtg_testkit::trigger_cards::{aimed_at, creature_with, trigger, watcher, when};
use mtg_testkit::{Driver, Scenario, ScriptError, ready};

fn creatures() -> Targetable {
    Targetable::Object(Selector(vec![Pred::Type(CardType::Creature)]))
}

/// The names of the creatures on the battlefield.
fn alive(state: &GameState) -> Vec<String> {
    state
        .zone(mtg_engine::zones::Zone::Battlefield)
        .iter()
        .filter_map(|id| state.object(*id))
        .map(|o| o.card().name.clone())
        .collect()
}

/// Spark: "Whenever a creature dies, Spark deals 1 damage to target creature" -- aimed by its controller, who always
/// picks the first creature that is not Spark, if there is one.
fn spark() -> std::sync::Arc<mtg_cards::CardDefinition> {
    creature_with(
        "Spark",
        vec![aimed_at(
            trigger(when::any_creature_dies(), vec![damage(Ref::Target(0), 1)]),
            creatures(),
        )],
    )
}

#[test]
fn a_chain_of_deaths_and_triggers_goes_on_the_stack_without_priority_in_between()
-> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(2)
            .hand(0, [burn("Bolt", 3)])
            .battlefield(0, [ready(mountain()), ready(spark())])
            .battlefield(
                1,
                [
                    ready(free_creature("A", 1, 1)),
                    ready(free_creature("B", 1, 1)),
                    ready(free_creature("C", 1, 1)),
                ],
            ),
    )?;
    let target = driver.target_named("A")?;
    let mut log = driver.cast_at("Bolt", &[target])?;
    // Each time Spark's trigger asks for a target, aim at the creature that is not Spark, else at Spark.
    for _ in 0..100 {
        if alive(driver.state()) == ["Mountain"] && driver.state().stack().is_empty() {
            break;
        }
        match &driver.decision()?.kind {
            DecisionKind::ChooseTargets(options) => {
                let spark = driver.target_named("Spark")?;
                let pick = options
                    .legal
                    .iter()
                    .copied()
                    .find(|t| *t != spark)
                    .unwrap_or(spark);
                log.extend(driver.choose_targets(&[pick])?);
            }
            DecisionKind::Priority(_) => log.extend(driver.pass()?),
            other => panic!("{}", other.name()),
        }
    }
    // Bolt killed A; Spark's trigger killed B; the next killed C; the next killed Spark, whose own death triggered once
    // more with nothing left to aim at.
    assert_eq!(alive(driver.state()), ["Mountain"]);
    let deaths = log
        .iter()
        .filter(|e| matches!(e, Event::CreatureDied { .. }))
        .count();
    assert_eq!(deaths, 4);
    assert_eq!(
        log.iter()
            .filter(|e| matches!(e, Event::TriggerRemoved { .. }))
            .count(),
        1,
        "the last trigger has nothing to aim at"
    );

    // Between a death and the trigger it causes going on the stack, nobody was given priority.
    for (at, event) in log.iter().enumerate() {
        if !matches!(event, Event::CreatureDied { .. }) {
            continue;
        }
        let rest = &log[at..];
        let next_trigger = rest
            .iter()
            .position(|e| matches!(e, Event::AbilityPutOnStack(put) if put.cause.is_some()))
            .or_else(|| {
                rest.iter()
                    .position(|e| matches!(e, Event::TriggerRemoved { .. }))
            })
            .unwrap();
        assert!(
            !rest[..next_trigger].iter().any(|e| matches!(
                e,
                Event::PriorityGranted { .. } | Event::PriorityPassed { .. }
            )),
            "priority between the death at {at} and its trigger: {:?}",
            &rest[..=next_trigger]
        );
    }
    Ok(())
}

#[test]
fn a_trigger_set_off_while_others_are_put_on_the_stack_waits_for_the_next_round()
-> Result<(), ScriptError> {
    // Seat 0 has two triggers when a creature dies, one of them aimed at the Guard; Guard "becomes the target" of
    // it, which triggers the Guard's own ability while seat 1 still has a trigger of an older round waiting.
    let guard = watcher("Guard", when::this_becomes_a_target(), vec![gain_life(100)]);
    let theirs = watcher("Theirs", when::any_creature_dies(), vec![gain_life(10)]);
    let aimed = creature_with(
        "Aimed",
        vec![aimed_at(
            trigger(when::any_creature_dies(), vec![damage(Ref::Target(0), 0)]),
            creatures(),
        )],
    );
    let mut driver = Driver::from_scenario(
        Scenario::new(2)
            .hand(0, [burn("Bolt", 3)])
            .battlefield(0, [ready(mountain()), ready(aimed)])
            .battlefield(
                1,
                [
                    ready(free_creature("Victim", 1, 1)),
                    ready(guard),
                    ready(theirs),
                ],
            ),
    )?;
    let victim = driver.target_named("Victim")?;
    driver.cast_at("Bolt", &[victim])?;
    driver.pass()?;
    driver.pass()?;
    // Seat 0 aims its trigger at the Guard.
    let at = driver.target_named("Guard")?;
    let mut events = driver.choose_targets(&[at])?;
    while !driver.state().stack().is_empty() {
        events.extend(driver.pass()?);
    }
    let puts: Vec<String> = events
        .iter()
        .filter_map(|e| match e {
            Event::AbilityPutOnStack(put) if put.cause.is_some() => Some(
                driver
                    .state()
                    .object(put.source.id)
                    .unwrap()
                    .card()
                    .name
                    .clone(),
            ),
            _ => None,
        })
        .collect();
    // Aimed (round one, active player), Theirs (round one, the other player), then Guard (round two: it triggered
    // when Aimed went on the stack, after the sweep began).
    assert_eq!(puts, ["Aimed", "Theirs", "Guard"], "{puts:?}");
    Ok(())
}

#[test]
fn the_triggers_of_a_player_who_leaves_while_they_wait_are_dropped() -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(3)
            .hand(0, [burn("Bolt", 3)])
            .battlefield(
                0,
                [
                    ready(mountain()),
                    ready(watcher(
                        "One",
                        when::any_creature_dies(),
                        vec![gain_life(1)],
                    )),
                    ready(watcher(
                        "Two",
                        when::any_creature_dies(),
                        vec![gain_life(10)],
                    )),
                ],
            )
            .battlefield(
                1,
                [
                    ready(free_creature("Victim", 1, 1)),
                    ready(watcher(
                        "Gone",
                        when::any_creature_dies(),
                        vec![gain_life(100)],
                    )),
                ],
            ),
    )?;
    let victim = driver.target_named("Victim")?;
    driver.cast_at("Bolt", &[victim])?;
    for _ in 0..3 {
        driver.pass()?;
    }
    // Seat 0 is asked to order its two; seat 1 and its trigger wait behind it, and seat 1 concedes.
    assert_eq!(driver.trigger_options()?.triggers.len(), 2);
    let events = driver.concede(pid(1))?;
    assert!(
        driver
            .state()
            .pending_triggers()
            .iter()
            .all(|w| w.trigger().controller == pid(0)),
        "only seat 0's remain: {events:?}"
    );
    driver.order_triggers(&[0, 1])?;
    while !driver.state().stack().is_empty() {
        driver.pass()?;
    }
    driver.expect_life(0, 31)?;
    driver.expect_life(2, 20)?;
    Ok(())
}

#[test]
fn a_trigger_of_a_source_that_left_the_game_is_dropped_too() -> Result<(), ScriptError> {
    // The Watcher's owner concedes after it died and its trigger waits for the sweep to put it on the stack.
    let mut driver = Driver::from_scenario(
        Scenario::new(3)
            .at(1, 0, Step::Main1)
            .hand(0, [burn("Bolt", 3)])
            .battlefield(0, [ready(mountain()), ready(free_creature("Victim", 1, 1))])
            .battlefield(
                1,
                [ready(watcher(
                    "Mourner",
                    when::this_dies(),
                    vec![gain_life(5)],
                ))],
            )
            .battlefield(
                2,
                [ready(watcher(
                    "Other",
                    when::any_creature_dies(),
                    vec![gain_life(7)],
                ))],
            ),
    )?;
    let mourner = driver.target_named("Mourner")?;
    driver.cast_at("Bolt", &[mourner])?;
    driver.pass()?;
    driver.pass()?;
    driver.pass()?;
    // Both the Mourner's and the Other's triggers went on the stack; seat 1 leaves with the Mourner's still there.
    let events = driver.concede(pid(1))?;
    assert_ne!(events.len(), 0);
    while !driver.state().stack().is_empty() {
        driver.pass()?;
    }
    driver.expect_life(1, 20)?;
    driver.expect_life(2, 27)?;
    Ok(())
}
