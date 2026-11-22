//! Goal: a trigger condition worded "while" (`Condition::AtTrigger`) is checked only as the event happens: true then, the
//! ability resolves though the condition is gone by then; false then, nothing triggers; an intervening "if" in the same
//! spot is checked again and does nothing (CR 603.4), at two to four seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common::pid;
use mtg_cards::refs::{Pred, Ref, Selector};
use mtg_cards::value::{Amount, Cmp, Condition, StatKind};
use mtg_core::card::CardType;
use mtg_engine::events::ResolutionOutcome;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_engine::{Event, GameState};
use mtg_testkit::cards::{burn, draw, free_creature, mountain};
use mtg_testkit::trigger_cards::{creature_with, only_if, trigger, when};
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// "creatures you control have total power 3 or greater".
fn total_power_three() -> Condition {
    Condition::Compare {
        left: Amount::Total {
            of: Selector(vec![
                Pred::Type(CardType::Creature),
                Pred::ControlledBy(Ref::You),
            ]),
            stat: StatKind::Power,
        },
        op: Cmp::Ge,
        right: Amount::Fixed(3),
    }
}

fn triggered(events: &[Event], state: &GameState) -> usize {
    events
        .iter()
        .filter(|event| match event {
            Event::AbilityTriggered(t) => state
                .object(t.source.id)
                .is_some_and(|o| o.card().name == "Raider"),
            _ => false,
        })
        .count()
}

fn trigger_outcome(events: &[Event]) -> Vec<ResolutionOutcome> {
    events
        .iter()
        .filter_map(|event| match event {
            Event::ResolutionEnded { outcome, .. } => Some(*outcome),
            _ => None,
        })
        .collect()
}

/// "Whenever Raider attacks while (or, if) creatures you control have total power 3 or greater, draw a card", with an
/// Ox of `ox_power` beside the 1/1 Raider; the Ox is burned in response to whatever triggers.
#[rstest]
#[case::while_true_then_gone_still_resolves(2, 2, true, 1, &[ResolutionOutcome::Resolved, ResolutionOutcome::Resolved])]
#[case::while_true_then_gone_still_resolves_three_seats(3, 2, true, 1, &[ResolutionOutcome::Resolved, ResolutionOutcome::Resolved])]
#[case::while_true_then_gone_still_resolves_four_seats(4, 2, true, 1, &[ResolutionOutcome::Resolved, ResolutionOutcome::Resolved])]
#[case::an_intervening_if_lapses(2, 2, false, 1, &[ResolutionOutcome::Resolved, ResolutionOutcome::Lapsed])]
#[case::an_intervening_if_lapses_four_seats(4, 2, false, 1, &[ResolutionOutcome::Resolved, ResolutionOutcome::Lapsed])]
#[case::while_false_never_triggers(2, 1, true, 0, &[ResolutionOutcome::Resolved])]
#[case::while_false_never_triggers_three_seats(3, 1, true, 0, &[ResolutionOutcome::Resolved])]
#[case::while_false_never_triggers_four_seats(4, 1, true, 0, &[ResolutionOutcome::Resolved])]
fn r116_06_a_while_condition_is_checked_only_as_the_event_happens(
    #[case] seats: usize,
    #[case] ox_power: i32,
    #[case] at_trigger_only: bool,
    #[case] triggers: usize,
    #[case] outcomes: &[ResolutionOutcome],
) -> Result<(), ScriptError> {
    let condition = if at_trigger_only {
        Condition::AtTrigger(Box::new(total_power_three()))
    } else {
        total_power_three()
    };
    let raider = creature_with(
        "Raider",
        vec![only_if(
            trigger(when::this_attacks(), vec![draw(1)]),
            condition,
        )],
    );
    let mut driver = Driver::from_scenario(
        Scenario::new(seats)
            .at(2, 0, Step::DeclareAttackers)
            .hand(0, [burn("Bolt", 3)])
            .battlefield(
                0,
                [
                    ready(mountain()),
                    ready(raider),
                    ready(free_creature("Ox", ox_power, 2)),
                ],
            ),
    )?;
    let hand = driver.state().zone(Zone::Hand(pid(0))).len();
    let mut events = driver.attack(&[("Raider", seats - 1)])?;
    assert_eq!(triggered(&events, driver.state()), triggers);
    // The Ox dies in response, so the total power is 1 as the trigger resolves.
    let ox = driver.target_named("Ox")?;
    events = driver.cast_at("Bolt", &[ox])?;
    while !driver.state().stack().is_empty() {
        events.extend(driver.pass()?);
    }
    assert_eq!(trigger_outcome(&events), outcomes);
    let drawn = usize::from(outcomes.last() == Some(&ResolutionOutcome::Resolved) && triggers == 1);
    assert_eq!(
        driver.state().zone(Zone::Hand(pid(0))).len(),
        hand - 1 + drawn
    );
    Ok(())
}
