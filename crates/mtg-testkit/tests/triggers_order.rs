//! Goal: the triggered abilities that wait for the stack go on it before a player would receive priority, the active player's first and then each other player's in turn order, for two to four seats and whoever is active; a player with two or more of them is asked how to order them only when the order can matter (the triggers of one source and one ability are not asked about), and the order they choose is the order they go on the stack in, the last on top.
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use common::pid;
use mtg_core::ids::PlayerId;
use mtg_engine::decision::DecisionKind;
use mtg_engine::turn::Step;
use mtg_engine::{Event, GameState};
use mtg_testkit::cards::{burn, free_creature, gain_life, mountain, twin_burn};
use mtg_testkit::trigger_cards::{trigger, watcher, when};
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// The life a watcher of `seat` gains: a different amount for each seat, so the order of the gains names the order of
/// the resolutions.
const fn gain(seat: usize) -> i32 {
    [1, 10, 100, 1000][seat]
}

/// A table of `seats` where `active` holds a Bolt and a Mountain, every seat has a watcher of any creature dying and
/// the seat after `active` has a Victim to burn.
fn table(seats: usize, active: usize) -> Scenario {
    let victim = (active + 1) % seats;
    let mut scenario = Scenario::new(seats)
        .at(1, active, Step::Main1)
        .hand(active, [burn("Bolt", 3)]);
    for seat in 0..seats {
        let mut permanents = vec![ready(watcher(
            &format!("Watcher {seat}"),
            when::any_creature_dies(),
            vec![gain_life(gain(seat))],
        ))];
        if seat == active {
            permanents.push(ready(mountain()));
        }
        if seat == victim {
            permanents.push(ready(free_creature("Victim", 1, 1)));
        }
        scenario = scenario.battlefield(seat, permanents);
    }
    scenario
}

/// Passes until the stack is empty and returns every event of the way.
fn resolve_everything(driver: &mut Driver) -> Result<Vec<Event>, ScriptError> {
    let mut events = Vec::new();
    while !driver.state().stack().is_empty() {
        events.extend(driver.pass()?);
    }
    Ok(events)
}

/// The controllers of the triggered abilities in the order they were put on the stack.
fn put_order(events: &[Event]) -> Vec<PlayerId> {
    events
        .iter()
        .filter_map(|event| match event {
            Event::AbilityPutOnStack(put) if put.cause.is_some() => Some(put.controller),
            _ => None,
        })
        .collect()
}

/// The players in the order they gained life.
fn gain_order(events: &[Event]) -> Vec<(PlayerId, u32)> {
    events
        .iter()
        .filter_map(|event| match event {
            Event::LifeGained { player, amount } => Some((*player, *amount)),
            _ => None,
        })
        .collect()
}

/// The names of the sources of the abilities on the stack, bottom first.
fn stack_sources(state: &GameState) -> Vec<String> {
    state
        .stack()
        .iter()
        .map(|id| {
            let object = state.object(*id).unwrap();
            let source = object.ability().unwrap().source().id;
            state.object(source).unwrap().card().name.clone()
        })
        .collect()
}

#[rstest]
#[case::two_seats_the_first_active(2, 0)]
#[case::two_seats_the_second_active(2, 1)]
#[case::three_seats_the_first_active(3, 0)]
#[case::three_seats_the_last_active(3, 2)]
#[case::four_seats_the_first_active(4, 0)]
#[case::four_seats_the_second_active(4, 1)]
#[case::four_seats_the_last_active(4, 3)]
fn the_active_players_trigger_goes_on_the_stack_first_then_the_others_in_turn_order(
    #[case] seats: usize,
    #[case] active: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table(seats, active))?;
    let victim = driver.target_named("Victim")?;
    let mut events = driver.cast_at("Bolt", &[victim])?;
    events.extend(resolve_everything(&mut driver)?);

    let turn_order: Vec<PlayerId> = (0..seats).map(|n| pid((active + n) % seats)).collect();
    assert_eq!(put_order(&events), turn_order, "active player first");
    let reversed: Vec<(PlayerId, u32)> = turn_order
        .iter()
        .rev()
        .map(|p| (*p, u32::try_from(gain(p.index())).unwrap()))
        .collect();
    assert_eq!(
        gain_order(&events),
        reversed,
        "the last put on the stack resolves first"
    );
    assert!(
        !events
            .iter()
            .any(|e| matches!(e, Event::TriggersOrdered { .. })),
        "a player with one trigger is never asked to order it"
    );
    Ok(())
}

#[rstest]
#[case::in_the_order_they_triggered(&[0, 1], &["First", "Second", "Third"], &[100, 10, 1])]
#[case::the_other_way_round(&[1, 0], &["Second", "First", "Third"], &[100, 1, 10])]
fn a_player_with_two_different_triggers_orders_them_and_the_last_on_the_stack_resolves_first(
    #[case] order: &[usize],
    #[case] stack: &[&str],
    #[case] gains: &[u32],
) -> Result<(), ScriptError> {
    let mut watchers = Vec::new();
    for (name, life) in [("First", 1), ("Second", 10)] {
        watchers.push(ready(watcher(
            name,
            when::any_creature_dies(),
            vec![gain_life(life)],
        )));
    }
    watchers.push(ready(mountain()));
    let third = watcher("Third", when::any_creature_dies(), vec![gain_life(100)]);
    let mut driver = Driver::from_scenario(
        Scenario::new(2)
            .hand(0, [burn("Bolt", 3)])
            .battlefield(0, watchers)
            .battlefield(1, [ready(free_creature("Victim", 1, 1)), ready(third)]),
    )?;
    let victim = driver.target_named("Victim")?;
    driver.cast_at("Bolt", &[victim])?;
    driver.pass()?;
    driver.pass()?;

    // Seat 0 is active and has two triggers of different sources: it is asked, before anything is on the stack.
    let decision = driver.decision()?;
    assert_eq!(decision.player, pid(0));
    assert_eq!(driver.state().stack(), [], "nothing is on the stack yet");
    let options = driver.trigger_options()?.clone();
    let names: Vec<_> = options
        .triggers
        .iter()
        .map(|t| {
            driver
                .state()
                .object(t.source.id)
                .unwrap()
                .card()
                .name
                .clone()
        })
        .collect();
    assert_eq!(names, ["First", "Second"], "in the order they triggered");
    assert!(
        options
            .triggers
            .iter()
            .all(|t| t.when == "Whenever a creature dies"),
        "{:?}",
        options.triggers
    );

    let events = driver.order_triggers(order)?;
    assert!(events.contains(&Event::TriggersOrdered {
        player: pid(0),
        order: order.to_vec()
    }));
    // The other seat's trigger goes on top, and nobody is asked again.
    assert_eq!(stack_sources(driver.state()), stack);
    assert!(matches!(driver.decision()?.kind, DecisionKind::Priority(_)));
    let events = resolve_everything(&mut driver)?;
    let amounts: Vec<u32> = gain_order(&events).iter().map(|(_, n)| *n).collect();
    assert_eq!(amounts, gains);
    Ok(())
}

#[test]
fn a_player_who_is_not_active_orders_theirs_after_the_active_players_are_on_the_stack()
-> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(2)
            .hand(0, [burn("Bolt", 3)])
            .battlefield(
                0,
                [
                    ready(mountain()),
                    ready(watcher(
                        "Mine",
                        when::any_creature_dies(),
                        vec![gain_life(1)],
                    )),
                ],
            )
            .battlefield(
                1,
                [
                    ready(free_creature("Victim", 1, 1)),
                    ready(watcher(
                        "Theirs A",
                        when::any_creature_dies(),
                        vec![gain_life(10)],
                    )),
                    ready(watcher(
                        "Theirs B",
                        when::any_creature_dies(),
                        vec![gain_life(100)],
                    )),
                ],
            ),
    )?;
    let victim = driver.target_named("Victim")?;
    driver.cast_at("Bolt", &[victim])?;
    driver.pass()?;
    driver.pass()?;
    assert_eq!(driver.decision()?.player, pid(1), "the opponent is asked");
    assert_eq!(
        stack_sources(driver.state()),
        ["Mine"],
        "the active player's trigger is on the stack already"
    );
    driver.order_triggers(&[1, 0])?;
    assert_eq!(
        stack_sources(driver.state()),
        ["Mine", "Theirs B", "Theirs A"]
    );
    Ok(())
}

#[test]
fn the_triggers_of_one_source_and_one_ability_are_not_asked_about_and_go_on_in_the_order_they_triggered()
-> Result<(), ScriptError> {
    // Two creatures die together, so the one watcher triggers twice; the order cannot matter.
    let mut driver = Driver::from_scenario(
        Scenario::new(2)
            .hand(0, [twin_burn("Twin")])
            .battlefield(
                0,
                [
                    ready(mountain()),
                    ready(mountain()),
                    ready(mountain()),
                    ready(watcher(
                        "Pair",
                        when::any_creature_dies(),
                        vec![gain_life(1)],
                    )),
                ],
            )
            .battlefield(
                1,
                [
                    ready(free_creature("Victim A", 1, 1)),
                    ready(free_creature("Victim B", 1, 1)),
                ],
            ),
    )?;
    let (a, b) = (
        driver.target_named("Victim A")?,
        driver.target_named("Victim B")?,
    );
    driver.begin_cast("Twin")?;
    driver.choose_targets(&[a])?;
    driver.choose_targets(&[b])?;
    driver.answer(mtg_engine::Answer::PayMana(
        mtg_engine::decision::PayChoice::Auto,
    ))?;
    let mut events = Vec::new();
    for _ in 0..2 {
        events.extend(driver.pass()?);
    }
    assert!(
        matches!(driver.decision()?.kind, DecisionKind::Priority(_)),
        "no order is asked for"
    );
    assert_eq!(driver.state().stack().len(), 2);
    events.extend(resolve_everything(&mut driver)?);
    assert!(
        !events
            .iter()
            .any(|e| matches!(e, Event::TriggersOrdered { .. }))
    );
    let triggered: Vec<_> = events
        .iter()
        .filter_map(|e| match e {
            Event::AbilityTriggered(t) => Some(t.as_ref()),
            _ => None,
        })
        .collect();
    assert_eq!(triggered.len(), 2);
    assert_eq!(triggered[0].source.id, triggered[1].source.id);
    assert_eq!(
        put_order(&events),
        [pid(0), pid(0)],
        "both went on the stack"
    );
    // The first to trigger is the first on the stack, so it resolves last.
    let puts: Vec<_> = events
        .iter()
        .filter_map(|e| match e {
            Event::AbilityPutOnStack(put) if put.cause.is_some() => {
                Some(put.cause.clone().unwrap())
            }
            _ => None,
        })
        .collect();
    assert_eq!(puts[0], triggered[0].cause);
    assert_eq!(puts[1], triggered[1].cause);
    Ok(())
}

#[test]
fn two_abilities_of_one_source_that_trigger_together_are_asked_about() -> Result<(), ScriptError> {
    let twins = mtg_testkit::trigger_cards::creature_with(
        "Twins",
        vec![
            trigger(when::any_creature_dies(), vec![gain_life(1)]),
            trigger(when::any_creature_dies(), vec![gain_life(10)]),
        ],
    );
    let mut driver = Driver::from_scenario(
        Scenario::new(2)
            .hand(0, [burn("Bolt", 3)])
            .battlefield(0, [ready(mountain()), ready(twins)])
            .battlefield(1, [ready(free_creature("Victim", 1, 1))]),
    )?;
    let victim = driver.target_named("Victim")?;
    driver.cast_at("Bolt", &[victim])?;
    driver.pass()?;
    driver.pass()?;
    let options = driver.trigger_options()?;
    assert_eq!(
        options
            .triggers
            .iter()
            .map(|t| t.ability)
            .collect::<Vec<_>>(),
        [0, 1]
    );
    Ok(())
}
