//! Goal: replacement effects that one batch of simultaneous events applies each name what they make by an id of their own at every table size (each Firestorm Phoenix a wipe destroys returns to its owner's hand with a play rule of its own, each creature Valentin exiles gets its own reflexive trigger), so the game goes on and the log replays.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use common::removal::corpus;
use common::{name_of, pid};
use mtg_engine::rules::Rule;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_engine::{Event, replay};
use mtg_testkit::cards::{plains, swamp};
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// The ids of the continuous effects and rule changes the log created, in order.
fn effect_ids(events: &[Event]) -> Vec<u64> {
    events
        .iter()
        .filter_map(|event| match event {
            Event::EffectCreated { effect } => Some(effect.id.0),
            Event::RuleCreated { effect } => Some(effect.id.0),
            _ => None,
        })
        .collect()
}

/// The names of the cards in `zone`, sorted.
fn names(driver: &Driver, zone: Zone) -> Vec<String> {
    let state = driver.state();
    let mut names: Vec<String> = state
        .zone(zone)
        .iter()
        .map(|id| name_of(state, *id))
        .collect();
    names.sort();
    names
}

/// Answers every question passively and passes until the stack is empty and seat 0 has priority again.
fn settle(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..300 {
        let decision = driver.decision()?;
        if driver.state().stack().is_empty() && decision.kind.name() == "priority" {
            return Ok(());
        }
        if decision.kind.name() == "priority" {
            driver.pass()?;
        } else {
            driver.answer_passively()?;
        }
    }
    Err(ScriptError::Stuck("the stack".to_owned()))
}

/// Seat 0 casts `wipe` with `count` copies of `card` under each other seat's control (Valentin under seat 0's when
/// the card is the Dean): each replacement of the batch makes its own rule or trigger, the game goes on and replays.
#[rstest]
fn each_replacement_of_a_wipe_makes_its_own(
    #[values(2, 3, 4)] seats: usize,
    #[values("Wrath of God", "Day of Judgment", "Bontu's Last Reckoning")] wipe: &str,
    #[values("Firestorm Phoenix", "Valentin, Dean of the Vein")] card: &str,
    #[values(1, 2)] count: usize,
) -> Result<(), ScriptError> {
    let valentin = card.starts_with("Valentin");
    let mut permanents: Vec<_> = (0..4).map(|_| ready(plains())).collect();
    permanents.extend((0..3).map(|_| ready(swamp())));
    if valentin {
        permanents.push(ready(corpus(
            "Valentin, Dean of the Vein // Lisette, Dean of the Root",
        )));
    }
    let mut scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .hand_named(0, [wipe])
        .battlefield(0, permanents);
    let theirs = if valentin {
        "Grizzly Bears"
    } else {
        "Firestorm Phoenix"
    };
    for seat in 1..seats {
        scenario = scenario.battlefield_named(seat, vec![theirs; count]);
    }
    let mut driver = Driver::from_scenario(scenario)?;
    driver.cast(wipe)?;
    settle(&mut driver)?;
    let made = (seats - 1) * count;
    for seat in 1..seats {
        let back = names(&driver, Zone::Hand(pid(seat)));
        if valentin {
            assert!(back.is_empty(), "seat {seat}'s Bears were exiled");
        } else {
            assert_eq!(
                back,
                vec![theirs; count],
                "seat {seat}'s Phoenixes came back"
            );
        }
    }
    if valentin {
        assert_eq!(
            names(&driver, Zone::Exile),
            vec![theirs; made],
            "each exiled"
        );
    } else {
        let plays = driver
            .state()
            .rule_effects()
            .iter()
            .filter(|effect| matches!(effect.rule, Rule::R119PlaysRevealed(_)))
            .count();
        assert_eq!(plays, made, "a play rule for each Phoenix");
        let ids = effect_ids(driver.engine().events());
        let mut distinct = ids.clone();
        distinct.sort_unstable();
        distinct.dedup();
        assert_eq!(distinct, ids, "each rule has an id of its own");
    }
    let state = replay(driver.engine().setup(), driver.engine().events()).unwrap();
    assert_eq!(state.state_hash(), driver.state().state_hash());
    Ok(())
}
