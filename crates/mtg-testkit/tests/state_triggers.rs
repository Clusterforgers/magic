//! Goal: a state trigger (CR 603.8) triggers once when its state is true, wherever the game finds it (at its start, after a state-based action, after an ability that changes it), goes on the stack before anyone receives priority, does not trigger again while its ability waits for the stack or is on it, triggers again once the ability has left the stack and the state is true again, and is judged for the player who controls its source; and every game replays to the same state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use common::{names, pid};
use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_engine::decision::DecisionKind;
use mtg_engine::object::ObjectKind;
use mtg_engine::zones::Zone;
use mtg_engine::{Event, replay};
use mtg_testkit::cards::{free_creature, mountain};
use mtg_testkit::corpus::card;
use mtg_testkit::zone_cards::{hollow_scholar, lone_wolf};
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;
use std::sync::Arc;

/// Passes priority while the stack holds something and the decision is a priority one.
fn pass_while_resolving(driver: &mut Driver) -> Result<Vec<Event>, ScriptError> {
    let mut events = Vec::new();
    for _ in 0..60 {
        let priority = matches!(driver.decision()?.kind, DecisionKind::Priority(_));
        if !priority || driver.state().stack().is_empty() {
            break;
        }
        events.extend(driver.pass()?);
    }
    Ok(events)
}

fn triggered(events: &[Event]) -> usize {
    events
        .iter()
        .filter(|e| matches!(e, Event::AbilityTriggered(_)))
        .count()
}

fn assert_replays(driver: &Driver) {
    let state = replay(driver.engine().setup(), driver.engine().events()).unwrap();
    assert_eq!(state.state_hash(), driver.state().state_hash());
}

/// An artifact that can be sacrificed for nothing but a little life.
fn gem() -> Arc<CardDefinition> {
    let ability: Ability = serde_json::from_str(
        r#"{"Activated":{"cost":["SacrificeSelf"],"effects":[{"GainLife":{"amount":1}}]}}"#,
    )
    .unwrap();
    Arc::new(
        CardDefinition::new("Gem")
            .with_type(CardType::Artifact)
            .with_rules(ability),
    )
}

#[test]
fn a_state_trigger_triggers_when_the_game_begins_in_its_state_and_its_ability_sacrifices_the_source()
-> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(2).battlefield(0, [ready(card("Covetous Dragon").unwrap())]),
    )?;
    // No artifact: the trigger is on the stack before the first player receives priority.
    assert_eq!(driver.state().stack().len(), 1);
    assert!(driver.state().pending_triggers().is_empty());
    assert_eq!(driver.state().turn().active(), pid(0));
    let events = pass_while_resolving(&mut driver)?;
    assert_eq!(names(driver.state(), Zone::Graveyard(pid(0))), ["Covetous Dragon"]);
    assert_eq!(triggered(&events), 0, "it triggered once, at the start");
    assert_eq!(
        triggered(driver.engine().events()),
        1,
        "and never again once the Dragon was gone"
    );
    assert_replays(&driver);
    Ok(())
}

#[test]
fn it_waits_while_the_state_is_false_and_triggers_the_moment_an_ability_makes_it_true()
-> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(2).battlefield(
            0,
            [ready(card("Covetous Dragon").unwrap()), ready(gem())],
        ),
    )?;
    assert!(driver.state().stack().is_empty(), "an artifact is in play");
    assert_eq!(triggered(driver.engine().events()), 0);
    // Sacrificing the artifact is a cost; its ability goes on the stack, and the Dragon's trigger above it.
    let events = driver.begin_activation("Gem")?;
    assert_eq!(triggered(&events), 1);
    let stack: Vec<ObjectKind> = driver
        .state()
        .stack()
        .iter()
        .map(|id| driver.state().object(*id).unwrap().kind())
        .collect();
    assert_eq!(stack, [ObjectKind::Ability, ObjectKind::Ability]);
    let top = driver.state().object(*driver.state().stack().last().unwrap()).unwrap();
    assert_eq!(
        driver.state().object(top.ability().unwrap().source().id).unwrap().card().name,
        "Covetous Dragon"
    );
    pass_while_resolving(&mut driver)?;
    assert_eq!(names(driver.state(), Zone::Graveyard(pid(0))), ["Gem", "Covetous Dragon"]);
    assert_eq!(driver.state().player(pid(0)).unwrap().life(), 21);
    assert_replays(&driver);
    Ok(())
}

#[test]
fn a_state_trigger_triggers_again_each_time_its_ability_has_left_the_stack_and_never_twice_at_once()
-> Result<(), ScriptError> {
    // "Whenever you have two or fewer cards in hand, draw a card": the hand is empty, so it draws three times.
    let library: Vec<_> = (0..6).map(|n| free_creature(&format!("Card {n}"), 1, 1)).collect();
    let mut driver = Driver::from_scenario(
        Scenario::new(2)
            .battlefield(0, [ready(hollow_scholar().unwrap())])
            .library(0, library),
    )?;
    let mut max_alive = 0;
    let mut draws = 0;
    for _ in 0..30 {
        let state = driver.state();
        let alive = state.pending_triggers().len()
            + state
                .stack()
                .iter()
                .filter(|id| state.object(**id).unwrap().kind() == ObjectKind::Ability)
                .count();
        max_alive = max_alive.max(alive);
        if state.stack().is_empty() {
            break;
        }
        draws += driver
            .pass()?
            .iter()
            .filter(|e| matches!(e, Event::CardDrawn { .. }))
            .count();
    }
    assert_eq!(max_alive, 1, "one instance at a time");
    assert_eq!(draws, 3);
    assert_eq!(driver.state().zone(Zone::Hand(pid(0))).len(), 3);
    assert_eq!(triggered(driver.engine().events()), 3);
    assert_replays(&driver);
    Ok(())
}

#[rstest]
#[case::alone(0, true)]
#[case::with_another_creature(1, false)]
#[case::with_two_others(2, false)]
fn the_state_is_judged_for_the_controller_of_the_source(
    #[case] others: usize,
    #[case] sacrificed: bool,
) -> Result<(), ScriptError> {
    // Seat 1's wolf has company of its own; the creatures of seat 0 matter to seat 0's wolf alone.
    let mine = std::iter::once(ready(lone_wolf().unwrap()))
        .chain((0..others).map(|n| ready(free_creature(&format!("Friend {n}"), 1, 1))));
    let mut driver = Driver::from_scenario(
        Scenario::new(2)
            .battlefield(0, mine)
            .battlefield(
                1,
                [ready(lone_wolf().unwrap()), ready(free_creature("Ally", 1, 1))],
            ),
    )?;
    assert_eq!(triggered(driver.engine().events()), usize::from(sacrificed));
    pass_while_resolving(&mut driver)?;
    let wolves = driver
        .state()
        .zone(Zone::Battlefield)
        .iter()
        .filter(|id| driver.state().object(**id).unwrap().card().name == "Lone Wolf")
        .count();
    assert_eq!(wolves, if sacrificed { 1 } else { 2 });
    assert_replays(&driver);
    Ok(())
}

#[test]
fn a_state_trigger_is_found_after_the_state_based_action_and_before_anyone_has_priority()
-> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(2)
            .hand(0, [card("Lightning Bolt").unwrap()])
            .battlefield(
                0,
                [
                    ready(mountain()),
                    ready(lone_wolf().unwrap()),
                    ready(card("Grizzly Bears").unwrap()),
                ],
            ),
    )?;
    assert!(driver.state().stack().is_empty(), "the Bears keep it company");
    let bears = common::on_battlefield(driver.state(), "Grizzly Bears").unwrap();
    let at = mtg_engine::choose::Target::Object(driver.state().object(bears).unwrap().reference());
    driver.cast_at("Lightning Bolt", &[at])?;
    driver.pass()?;
    let events = driver.pass()?;
    let kinds: Vec<&str> = events
        .iter()
        .filter_map(|e| match e {
            Event::CreatureDied { .. } => Some("died"),
            Event::AbilityTriggered(_) => Some("triggered"),
            _ => None,
        })
        .collect();
    let death = kinds.iter().position(|k| *k == "died").unwrap();
    let trigger = kinds.iter().position(|k| *k == "triggered").unwrap_or_else(|| panic!("{kinds:?} {:?}", driver.state().stack()));
    assert!(death < trigger, "{kinds:?}");
    assert!(matches!(driver.decision()?.kind, DecisionKind::Priority(_)));
    assert_eq!(driver.state().stack().len(), 1, "on the stack before the next priority");
    assert_replays(&driver);
    Ok(())
}
