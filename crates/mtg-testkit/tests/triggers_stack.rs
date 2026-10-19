//! Goal: a triggered ability is detected from the events of a game, waits in the queue until a player would receive priority, goes on the stack as an object another seat can respond to, chooses its targets as it goes on (and is removed when it cannot), resolves like any ability, and the log of all of it replays to the state it was played to.
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use common::pid;
use mtg_cards::refs::{Pred, Ref, Selector, Targetable};
use mtg_core::card::CardType;
use mtg_engine::decision::DecisionKind;
use mtg_engine::events::ResolutionOutcome;
use mtg_engine::object::ObjectKind;
use mtg_engine::zones::Zone;
use mtg_engine::{Event, replay};
use mtg_testkit::cards::{damage, draw, gain_life};
use mtg_testkit::trigger_cards::{aimed_at, creature_with, trigger, watcher, when};
use mtg_testkit::{Driver, Scenario, ScriptError};

fn triggered(events: &[Event]) -> usize {
    events
        .iter()
        .filter(|e| matches!(e, Event::AbilityTriggered(_)))
        .count()
}

#[test]
fn an_enters_trigger_goes_on_the_stack_after_the_spell_resolves_and_the_other_seat_can_respond()
-> Result<(), ScriptError> {
    let drifter = watcher("Drifter", when::this_enters(), vec![draw(2)]);
    let mut driver = Driver::from_scenario(Scenario::new(2).hand(0, [drifter]))?;
    let hand = driver.state().zone(Zone::Hand(pid(0))).len();
    driver.cast("Drifter")?;
    driver.pass()?;
    let events = driver.pass()?;
    assert_eq!(triggered(&events), 1, "the creature entering triggers it");
    // The trigger is on the stack, and the active player has priority with it there.
    let top = *driver.state().stack().last().unwrap();
    let ability = driver.state().object(top).unwrap();
    assert_eq!(ability.kind(), ObjectKind::Ability);
    assert_eq!(ability.controller(), pid(0));
    assert_eq!(driver.decision()?.player, pid(0));
    assert!(matches!(driver.decision()?.kind, DecisionKind::Priority(_)));
    driver.pass()?;
    assert_eq!(
        driver.decision()?.player,
        pid(1),
        "the other seat can respond to it"
    );
    let events = driver.pass()?;
    assert!(
        events.iter().any(|e| matches!(
            e,
            Event::ResolutionEnded {
                outcome: ResolutionOutcome::Resolved,
                ..
            }
        )),
        "{events:?}"
    );
    assert_eq!(driver.state().zone(Zone::Hand(pid(0))).len(), hand - 1 + 2);
    assert_eq!(driver.state().stack(), []);
    assert_eq!(driver.state().pending_triggers(), []);
    Ok(())
}

#[test]
fn a_trigger_chooses_its_targets_as_it_goes_on_the_stack_and_the_choice_is_logged()
-> Result<(), ScriptError> {
    let raider = aimed_at(
        trigger(when::this_enters(), vec![damage(Ref::Target(0), 2)]),
        Targetable::Opponent,
    );
    let card = creature_with("Raider", vec![raider]);
    let mut driver = Driver::from_scenario(Scenario::new(3).hand(0, [card]))?;
    driver.cast("Raider")?;
    for _ in 0..3 {
        driver.pass()?;
    }
    let DecisionKind::ChooseTargets(options) = &driver.decision()?.kind else {
        panic!("the trigger asks for its target");
    };
    assert!(options.triggered && !options.can_cancel);
    assert_eq!(options.wording, "target opponent");
    assert_eq!(options.legal.len(), 2, "both opponents");
    assert_eq!(driver.decision()?.player, pid(0));
    let at = driver.target_player(2);
    let events = driver.choose_targets(&[at])?;
    let Some(Event::AbilityPutOnStack(put)) = events
        .iter()
        .find(|e| matches!(e, Event::AbilityPutOnStack(_)))
    else {
        panic!("the trigger goes on the stack");
    };
    assert_eq!(put.choices.targets, [vec![at]]);
    assert!(put.cause.is_some());
    while !driver.state().stack().is_empty() {
        driver.pass()?;
    }
    driver.expect_life(2, 18)?;
    driver.expect_life(1, 20)?;
    Ok(())
}

#[test]
fn a_trigger_with_no_legal_target_is_removed_instead_of_going_on_the_stack()
-> Result<(), ScriptError> {
    // Nothing on the table is an artifact, so the ability has nothing to aim at (CR 603.3d).
    let aimed = aimed_at(
        trigger(when::this_enters(), vec![gain_life(1)]),
        Targetable::Object(Selector(vec![Pred::Type(CardType::Artifact)])),
    );
    let card = creature_with("Seeker", vec![aimed]);
    let mut driver = Driver::from_scenario(Scenario::new(2).hand(0, [card]))?;
    driver.cast("Seeker")?;
    driver.pass()?;
    let events = driver.pass()?;
    assert_eq!(triggered(&events), 1);
    assert!(
        events
            .iter()
            .any(|e| matches!(e, Event::TriggerRemoved { .. })),
        "{events:?}"
    );
    assert_eq!(driver.state().stack(), []);
    assert_eq!(driver.state().pending_triggers(), []);
    Ok(())
}

#[test]
fn the_log_of_a_game_with_triggers_replays_to_the_same_state_at_every_point()
-> Result<(), ScriptError> {
    let drifter = watcher("Drifter", when::this_enters(), vec![draw(1)]);
    let scenario = || Scenario::new(2).hand(0, [drifter.clone()]);
    let mut driver = Driver::from_scenario(scenario())?;
    driver.cast("Drifter")?;
    for _ in 0..4 {
        driver.pass()?;
    }
    let log = driver.engine().events();
    let setup = scenario().setup().unwrap();
    // Cut anywhere in the log -- before the trigger, while it waits, on the stack, resolved -- and the replay of the
    // prefix has the state the game had there.
    let mut played = mtg_engine::GameState::from_setup(&setup).unwrap();
    for (at, event) in log.iter().enumerate() {
        mtg_engine::apply_event(&mut played, event).unwrap();
        let replayed = replay(&setup, &log[..=at]).unwrap();
        assert_eq!(
            replayed.state_hash(),
            played.state_hash(),
            "after event {at}"
        );
    }
    assert_eq!(played.state_hash(), driver.state().state_hash());
    Ok(())
}
