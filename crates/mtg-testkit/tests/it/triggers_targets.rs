//! Goal: a triggered ability is a stack object like any other once it is there: its targets were chosen as it went on and are rechecked as it resolves (it does nothing if every one became illegal, CR 608.2b), an intervening "if" is checked again as it resolves (CR 603.4), a limit of once each turn holds for triggers and activations and resets with the turn, and a trigger whose controller or source has left the game is dropped.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use common::pid;
use mtg_cards::ability::{Ability, Timing};
use mtg_cards::cost::Cost;
use mtg_cards::effect::Effect;
use mtg_cards::refs::{Pred, Ref, Selector, Targetable, ZoneKind};
use mtg_cards::value::{Amount, Cmp, Condition};
use mtg_core::card::CardType;
use mtg_engine::events::ResolutionOutcome;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_engine::{Event, GameState};
use mtg_testkit::cards::{burn, damage, draw, free_creature, gain_life, mountain};
use mtg_testkit::trigger_cards::{
    aimed_at, artifact_with, creature_with, once_each_turn, only_if, trigger, watcher, when,
};
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

fn outcomes(events: &[Event]) -> Vec<ResolutionOutcome> {
    events
        .iter()
        .filter_map(|event| match event {
            Event::ResolutionEnded { outcome, .. } => Some(*outcome),
            _ => None,
        })
        .collect()
}

fn triggered_by(events: &[Event], name: &str, state: &GameState) -> usize {
    events
        .iter()
        .filter(|event| match event {
            Event::AbilityTriggered(t) => state
                .object(t.source.id)
                .is_some_and(|o| o.card().name == name),
            _ => false,
        })
        .count()
}

fn creatures() -> Targetable {
    Targetable::Object(Selector(vec![Pred::Type(CardType::Creature)]))
}

#[test]
fn a_trigger_whose_only_target_became_illegal_does_nothing() -> Result<(), ScriptError> {
    // Raider: "When this enters, it deals 3 damage to target creature."
    let raider = creature_with(
        "Raider",
        vec![aimed_at(
            trigger(when::this_enters(), vec![damage(Ref::Target(0), 3)]),
            creatures(),
        )],
    );
    let mut driver = Driver::from_scenario(
        Scenario::new(2)
            .hand(0, [raider])
            .hand(1, [burn("Bolt", 3)])
            .battlefield(1, [ready(mountain()), ready(free_creature("Guard", 1, 3))]),
    )?;
    driver.cast("Raider")?;
    driver.pass()?;
    driver.pass()?;
    // The trigger asks seat 0 for its target; the Raider itself and the Guard are the creatures there are.
    let guard = driver.target_named("Guard")?;
    let options = match &driver.decision()?.kind {
        mtg_engine::DecisionKind::ChooseTargets(options) => options.clone(),
        other => panic!("{}", other.name()),
    };
    assert_eq!(options.legal.len(), 2);
    assert_eq!(options.wording, "target creature");
    driver.choose_targets(&[guard])?;
    // The trigger is on the stack; seat 0 passes and seat 1 answers by killing its own Guard.
    driver.pass()?;
    assert_eq!(driver.decision()?.player, pid(1));
    let mut events = driver.cast_at("Bolt", &[guard])?;
    while !driver.state().stack().is_empty() {
        events.extend(driver.pass()?);
    }
    assert_eq!(
        outcomes(&events),
        [ResolutionOutcome::Resolved, ResolutionOutcome::Fizzled],
        "the Bolt resolved, then the trigger had no target left"
    );
    let raider = driver.id_of("Raider")?;
    let marked = driver
        .state()
        .object(raider)
        .and_then(|o| o.permanent())
        .map_or(99, mtg_engine::object::PermanentState::damage);
    assert_eq!(marked, 0, "the trigger did nothing");
    Ok(())
}

#[test]
fn a_trigger_resolves_against_the_targets_that_are_still_legal_when_it_has_two()
-> Result<(), ScriptError> {
    // Two target specifications: when one is gone the trigger still resolves for the other (CR 608.2b).
    let double = creature_with(
        "Double",
        vec![{
            let ability = aimed_at(
                trigger(
                    when::this_enters(),
                    vec![damage(Ref::Target(0), 2), damage(Ref::Target(1), 2)],
                ),
                creatures(),
            );
            aimed_at(ability, Targetable::Opponent)
        }],
    );
    let mut driver = Driver::from_scenario(
        Scenario::new(2)
            .hand(0, [double])
            .hand(1, [burn("Bolt", 3)])
            .battlefield(1, [ready(mountain()), ready(free_creature("Guard", 1, 3))]),
    )?;
    driver.cast("Double")?;
    driver.pass()?;
    driver.pass()?;
    let guard = driver.target_named("Guard")?;
    driver.choose_targets(&[guard])?;
    let foe = driver.target_player(1);
    driver.choose_targets(&[foe])?;
    driver.pass()?;
    let mut events = driver.cast_at("Bolt", &[guard])?;
    while !driver.state().stack().is_empty() {
        events.extend(driver.pass()?);
    }
    assert_eq!(
        outcomes(&events),
        [ResolutionOutcome::Resolved, ResolutionOutcome::Resolved]
    );
    driver.expect_life(1, 18)?;
    Ok(())
}

#[rstest]
#[case::the_condition_stays_true(20, ResolutionOutcome::Resolved, 1)]
#[case::the_condition_is_gone_by_the_time_it_resolves(12, ResolutionOutcome::Lapsed, 0)]
fn an_intervening_if_is_checked_again_as_the_trigger_resolves(
    #[case] life: i32,
    #[case] outcome: ResolutionOutcome,
    #[case] drawn: usize,
) -> Result<(), ScriptError> {
    // "At the beginning of your upkeep, if you have 10 or more life, draw a card."
    let tithe = artifact_with(
        "Tithe",
        vec![only_if(
            trigger(when::your_upkeep(), vec![draw(1)]),
            Condition::Compare {
                left: Amount::Life(Ref::You),
                op: Cmp::Ge,
                right: Amount::Fixed(10),
            },
        )],
    );
    let mut driver = Driver::from_scenario(
        Scenario::new(2)
            .at(2, 0, Step::Untap)
            .life(0, life)
            .hand(0, [burn("Bolt", 3)])
            .battlefield(0, [ready(mountain()), ready(tithe)]),
    )?;
    // The trigger is on the stack in the upkeep, with the condition true; the player burns themselves in response.
    assert_eq!(driver.state().turn().step(), Step::Upkeep);
    assert_eq!(driver.state().stack().len(), 1);
    let hand = driver.state().zone(Zone::Hand(pid(0))).len();
    let me = driver.target_player(0);
    let mut events = driver.cast_at("Bolt", &[me])?;
    while !driver.state().stack().is_empty() {
        events.extend(driver.pass()?);
    }
    assert_eq!(
        outcomes(&events),
        [ResolutionOutcome::Resolved, outcome],
        "the Bolt, then the trigger"
    );
    // The Bolt left the hand; a card was drawn if the trigger did something.
    assert_eq!(
        driver.state().zone(Zone::Hand(pid(0))).len(),
        hand - 1 + drawn
    );
    Ok(())
}

#[test]
fn an_ability_that_triggers_once_each_turn_triggers_once_and_again_in_the_next_turn()
-> Result<(), ScriptError> {
    let echo = creature_with(
        "Echo",
        vec![once_each_turn(trigger(
            when::any_creature_enters(),
            vec![gain_life(1)],
        ))],
    );
    let mut driver = Driver::from_scenario(
        Scenario::new(2)
            .hand(
                0,
                [
                    free_creature("First", 1, 1),
                    free_creature("Second", 1, 1),
                    free_creature("Third", 1, 1),
                ],
            )
            .battlefield(0, [ready(echo)]),
    )?;
    let mut events = Vec::new();
    for name in ["First", "Second"] {
        events.extend(driver.cast(name)?);
        while !driver.state().stack().is_empty() {
            events.extend(driver.pass()?);
        }
    }
    assert_eq!(
        triggered_by(&events, "Echo", driver.state()),
        1,
        "two creatures entered in the turn, and it triggered for the first"
    );
    // The next turn of seat 0 gives it back.
    driver.idle_until(3, Step::Main1)?;
    let events = driver.cast("Third")?;
    let mut all = events;
    while !driver.state().stack().is_empty() {
        all.extend(driver.pass()?);
    }
    assert_eq!(triggered_by(&all, "Echo", driver.state()), 1);
    Ok(())
}

#[test]
fn an_activated_ability_limited_to_once_each_turn_is_offered_once_and_again_next_turn()
-> Result<(), ScriptError> {
    let pump = artifact_with(
        "Pump",
        vec![once_each_turn(Ability::Activated {
            cost: Cost(Vec::new()),
            timing: Timing::Instant,
            condition: None,
            limit: None,
            from: ZoneKind::Battlefield,
            targets: Vec::new(),
            effects: vec![draw(1)],
        })],
    );
    let mut driver = Driver::from_scenario(Scenario::new(2).battlefield(0, [ready(pump)]))?;
    let offered = |driver: &Driver| match &driver.decision().unwrap().kind {
        mtg_engine::DecisionKind::Priority(options) => options.abilities.len(),
        other => panic!("{}", other.name()),
    };
    assert_eq!(offered(&driver), 1);
    driver.begin_activation("Pump")?;
    assert_eq!(offered(&driver), 0, "used for the turn");
    while !driver.state().stack().is_empty() {
        driver.pass()?;
    }
    assert_eq!(offered(&driver), 0, "still used after it resolved");
    driver.idle_until(3, Step::Main1)?;
    assert_eq!(
        offered(&driver),
        1,
        "a new turn of its controller gives it back"
    );
    Ok(())
}

#[test]
fn a_trigger_with_a_target_that_cannot_be_met_is_removed_and_the_game_goes_on()
-> Result<(), ScriptError> {
    // "When this dies, it deals 1 damage to target creature an opponent controls": there is none.
    let ghost = creature_with(
        "Ghost",
        vec![aimed_at(
            trigger(when::this_dies(), vec![damage(Ref::Target(0), 1)]),
            Targetable::Object(Selector(vec![
                Pred::Type(CardType::Creature),
                Pred::ControlledBy(Ref::Opponents),
            ])),
        )],
    );
    let mut driver = Driver::from_scenario(
        Scenario::new(2)
            .hand(1, [burn("Bolt", 3)])
            .battlefield(0, [ready(ghost)])
            .battlefield(1, [ready(mountain())])
            .at(1, 1, Step::Main1),
    )?;
    let ghost = driver.target_named("Ghost")?;
    let mut events = driver.cast_at("Bolt", &[ghost])?;
    while !driver.state().stack().is_empty() {
        events.extend(driver.pass()?);
    }
    assert_eq!(
        events
            .iter()
            .filter(|e| matches!(e, Event::TriggerRemoved { .. }))
            .count(),
        1,
        "{events:?}"
    );
    assert!(
        !events
            .iter()
            .any(|e| matches!(e, Event::AbilityPutOnStack(put) if put.cause.is_some())),
        "it never went on the stack"
    );
    Ok(())
}

#[test]
fn choosing_no_target_removes_a_trigger_and_a_spell_may_still_be_taken_back()
-> Result<(), ScriptError> {
    let raider = creature_with(
        "Raider",
        vec![aimed_at(
            trigger(when::this_enters(), vec![damage(Ref::Target(0), 1)]),
            creatures(),
        )],
    );
    let mut driver = Driver::from_scenario(Scenario::new(2).hand(0, [raider]))?;
    driver.cast("Raider")?;
    driver.pass()?;
    driver.pass()?;
    // The trigger cannot be backed out of with a cancel.
    let refused = driver.cancel_targets();
    assert!(refused.is_err(), "a trigger is not cancelled like a cast");
    let events = driver.remove_trigger()?;
    assert!(
        events
            .iter()
            .any(|e| matches!(e, Event::TriggerRemoved { .. }))
    );
    assert_eq!(driver.state().stack(), []);
    assert!(matches!(
        driver.decision()?.kind,
        mtg_engine::DecisionKind::Priority(_)
    ));
    Ok(())
}

#[test]
fn the_triggers_of_the_cleanup_discard_get_a_round_of_priority_before_the_turn_ends()
-> Result<(), ScriptError> {
    // At the cleanup step the player discards down to seven, and the triggers that causes get a round of priority
    // before the turn ends (CR 514.3a).
    let madness = watcher(
        "Madness",
        when::you_discard(),
        vec![Effect::GainLife {
            who: Ref::You,
            amount: Amount::Fixed(1),
        }],
    );
    let hand: Vec<_> = (0..9).map(|_| mtg_testkit::cards::forest()).collect();
    let mut driver = Driver::from_scenario(
        Scenario::new(2)
            .at(1, 0, Step::End)
            .hand(0, hand)
            .battlefield(0, [ready(madness)]),
    )?;
    driver.pass()?;
    driver.pass()?;
    let discard = driver.answer_passively()?;
    assert!(
        discard
            .iter()
            .filter(|e| matches!(e, Event::AbilityTriggered(_)))
            .count()
            == 2,
        "one for each card discarded: {discard:?}"
    );
    // The turn has not ended: the abilities wait on the stack in the cleanup step.
    assert_eq!(driver.state().turn().step(), Step::Cleanup);
    assert_eq!(driver.state().stack().len(), 2);
    let mut events = Vec::new();
    while driver.state().turn().number() == 1 {
        events.extend(driver.pass()?);
    }
    driver.expect_life(0, 22)?;
    let resolved = events
        .iter()
        .position(|e| matches!(e, Event::ResolutionEnded { .. }))
        .unwrap();
    let next_turn = events
        .iter()
        .position(|e| matches!(e, Event::TurnBegan { .. }))
        .unwrap();
    assert!(resolved < next_turn, "they resolved before the turn ended");
    Ok(())
}
