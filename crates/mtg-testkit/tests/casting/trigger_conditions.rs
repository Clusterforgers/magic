//! Goal: a triggered ability whose effects all hang on conditions that are false as it begins to resolve logs `ConditionsFailed` with the conditions in the card's words before it ends, and does nothing; one whose condition holds, or that does more than its conditions, logs no such line; at tables of two and four seats, whoever rolls, over several seeds, and the log replays.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::value::CounterKind;
use mtg_core::card::CardType;
use mtg_engine::Event;
use mtg_engine::events::ResolutionOutcome;
use mtg_engine::turn::Step;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::l291_dice::{l291_replays, l291_roll};

/// "Whenever a player rolls one or more dice, `effects`" as JSON.
fn watcher(effects: &str) -> Arc<CardDefinition> {
    let rules: Ability = serde_json::from_str(&format!(
        r#"{{"Triggered":{{"when":{{"DiceRolled":{{"who":"EachPlayer"}}}},"effects":{effects}}}}}"#
    ))
    .unwrap();
    Arc::new(
        CardDefinition::new("Watcher")
            .with_type(CardType::Artifact)
            .with_rules(rules),
    )
}

/// "If it's your turn, put a +1/+1 counter on this."
const YOUR_TURN: &str =
    r#"[{"If":{"condition":"YourTurn","then":[{"PutCounters":{"on":"This","counter":"+1/+1"}}]}}]"#;
/// The same with a counter put either way, so the ability does more than its condition.
const AND_MORE: &str = r#"[{"If":{"condition":"YourTurn","then":[{"PutCounters":{"on":"This","counter":"+1/+1"}}]}},{"PutCounters":{"on":"This","counter":"charge"}}]"#;
/// "If it's your turn, ...; otherwise, put a charge counter on this."
const OTHERWISE: &str = r#"[{"If":{"condition":"YourTurn","then":[{"PutCounters":{"on":"This","counter":"+1/+1"}}],"otherwise":[{"PutCounters":{"on":"This","counter":"charge"}}]}}]"#;

/// "{0}: Roll a six-sided die."
fn roller() -> Arc<CardDefinition> {
    let roll: Ability = serde_json::from_str(
        r#"{"Activated":{"cost":"{0}","effects":[{"RollDice":{"sides":6,"results":[]}}]}}"#,
    )
    .unwrap();
    Arc::new(
        CardDefinition::new("Die Roller")
            .with_type(CardType::Artifact)
            .with_rules(roll),
    )
}

#[rstest]
#[case::nothing_but_a_condition(YOUR_TURN, true)]
#[case::more_than_a_condition(AND_MORE, false)]
#[case::a_condition_with_an_otherwise(OTHERWISE, false)]
fn pf9g_a_trigger_whose_conditions_all_fail_says_why(
    #[case] effects: &str,
    #[case] all_conditions: bool,
    #[values(2, 4)] seats: usize,
    #[values(1, 2)] seed: u64,
    #[values(0, 1)] roller_seat: usize,
) -> Result<(), ScriptError> {
    let scenario = || {
        let scenario = Scenario::new(seats)
            .seed(seed)
            .at(2, roller_seat, Step::Main1);
        if roller_seat == 0 {
            scenario.battlefield(0, [ready(watcher(effects)), ready(roller())])
        } else {
            scenario
                .battlefield(roller_seat, [ready(roller())])
                .battlefield(0, [ready(watcher(effects))])
        }
    };
    let mut driver = Driver::from_scenario(scenario())?;
    let mark = driver.engine().events().len();
    l291_roll(&mut driver, 3)?;
    let events = &driver.engine().events()[mark..];
    let failed: Vec<&Event> = events
        .iter()
        .filter(|event| matches!(event, Event::ConditionsFailed { .. }))
        .collect();
    // The watcher's controller is on turn only when they roll.
    let fails = all_conditions && roller_seat != 0;
    assert_eq!(failed.len(), if fails { 3 } else { 0 }, "{failed:?}");
    for event in failed {
        let Event::ConditionsFailed { object, conditions } = event else {
            unreachable!()
        };
        assert_eq!(conditions, &vec!["it's your turn".to_owned()]);
        // The ability then ends as resolved, having done nothing.
        let at = events.iter().position(|e| e == event).unwrap();
        assert_eq!(
            events.get(at + 1),
            Some(&Event::ResolutionEnded {
                object: *object,
                outcome: ResolutionOutcome::Resolved,
            })
        );
    }
    let watcher_id = driver.id_of("Watcher")?;
    let plus = driver
        .state()
        .object(watcher_id)
        .and_then(|object| object.permanent())
        .map_or(0, |permanent| {
            permanent.counters_of(&CounterKind::PlusOnePlusOne)
        });
    assert_eq!(plus, if roller_seat == 0 { 3 } else { 0 });
    l291_replays(&driver, scenario());
    Ok(())
}
