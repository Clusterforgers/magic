//! Goal: a trigger worded "<event> during your turn" or "during an opponent's turn" (`Condition::AtTrigger` of the turn)
//! triggers only on that turn and then resolves: whose turn it is is checked as the event happens (CR 603.2), at two
//! to four seats, for the active player's ability and another seat's.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common::pid;
use mtg_cards::value::Condition;
use mtg_engine::events::ResolutionOutcome;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_engine::{Event, GameState};
use mtg_testkit::cards::{burn, draw, free_creature, mountain};
use mtg_testkit::trigger_cards::{creature_with, only_if, trigger, when};
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

fn triggered(events: &[Event], state: &GameState) -> usize {
    events
        .iter()
        .filter(|event| match event {
            Event::AbilityTriggered(t) => state
                .object(t.source.id)
                .is_some_and(|o| o.card().name == "Watcher"),
            _ => false,
        })
        .count()
}

/// "Whenever a creature dies during your turn (or an opponent's), draw a card" on seat `seat`; the active seat 0 burns
/// an Ox.
#[rstest]
#[case::your_turn_on_yours(2, 0, false, 1)]
#[case::your_turn_on_yours_three_seats(3, 0, false, 1)]
#[case::your_turn_on_yours_four_seats(4, 0, false, 1)]
#[case::your_turn_on_an_opponents(2, 1, false, 0)]
#[case::your_turn_on_an_opponents_four_seats(4, 3, false, 0)]
#[case::an_opponents_turn_on_theirs(2, 1, true, 1)]
#[case::an_opponents_turn_on_theirs_three_seats(3, 2, true, 1)]
#[case::an_opponents_turn_on_theirs_four_seats(4, 3, true, 1)]
#[case::an_opponents_turn_on_yours(2, 0, true, 0)]
#[case::an_opponents_turn_on_yours_three_seats(3, 0, true, 0)]
fn r117_04_the_turn_is_checked_as_the_event_happens(
    #[case] seats: usize,
    #[case] seat: usize,
    #[case] opponents_turn: bool,
    #[case] triggers: usize,
) -> Result<(), ScriptError> {
    let turn = if opponents_turn {
        Condition::Not(Box::new(Condition::YourTurn))
    } else {
        Condition::YourTurn
    };
    let watcher = creature_with(
        "Watcher",
        vec![only_if(
            trigger(when::any_creature_dies(), vec![draw(1)]),
            Condition::AtTrigger(Box::new(turn)),
        )],
    );
    let mut active = vec![ready(mountain()), ready(free_creature("Ox", 2, 2))];
    let mut scenario = Scenario::new(seats)
        .at(2, 0, Step::Main1)
        .hand(0, [burn("Bolt", 3)]);
    if seat == 0 {
        active.push(ready(watcher));
    } else {
        scenario = scenario.battlefield(seat, [ready(watcher)]);
    }
    let mut driver = Driver::from_scenario(scenario.battlefield(0, active))?;
    let hand = driver.state().zone(Zone::Hand(pid(seat))).len();
    let ox = driver.target_named("Ox")?;
    let mut events = driver.cast_at("Bolt", &[ox])?;
    while !driver.state().stack().is_empty() {
        events.extend(driver.pass()?);
    }
    assert_eq!(triggered(&events, driver.state()), triggers);
    let resolved = events
        .iter()
        .filter(|event| {
            matches!(
                event,
                Event::ResolutionEnded {
                    outcome: ResolutionOutcome::Resolved,
                    ..
                }
            )
        })
        .count();
    // The Bolt, and the trigger when it triggered.
    assert_eq!(resolved, 1 + triggers);
    let cast = usize::from(seat == 0);
    assert_eq!(
        driver.state().zone(Zone::Hand(pid(seat))).len(),
        hand - cast + triggers
    );
    Ok(())
}
