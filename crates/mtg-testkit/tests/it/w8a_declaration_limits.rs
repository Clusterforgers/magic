//! Goal: limits on the whole attack or block declaration play through the engine at tables of two, three and four
//! seats: Silent Arbiter lets one creature attack and one block, a declaration of more is refused by name and leaves no
//! trace, Caverns of Despair lets two of three creatures that must attack attack (any two, never one or a free one in
//! place of one that must), and bot games under Dueling Grounds never declare more than one attacker or blocker; every
//! answer keeps the invariants and the log replays to the state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_engine::decision::DecisionKind;
use mtg_engine::error::IllegalAnswer;
use mtg_engine::turn::Step;
use mtg_engine::{EngineError, Event};
use mtg_testkit::cards::free_creature;
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, RandomBot, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::copying::replays;
use crate::common::pid;
use crate::w7d_control_scenarios::w7d_oracle;

type W8aResult = Result<(), ScriptError>;

/// A 2/2 that attacks each combat if able.
fn w8a_berserker(name: &str) -> Arc<CardDefinition> {
    w7d_oracle(
        name,
        "{1}{R}",
        "Creature — Human Berserker",
        Some((2, 2)),
        "This creature attacks each combat if able.",
    )
}

/// Whether `result` is the refusal of a declaration over the limit `allowed`.
fn w8a_too_many(result: &Result<Vec<Event>, ScriptError>, allowed: usize) -> bool {
    matches!(
        result,
        Err(ScriptError::Engine(EngineError::Illegal(IllegalAnswer::TooManyCreatures { allowed: a, .. })))
            if *a == allowed
    )
}

/// Whether `result` is the refusal of a declaration that leaves out a creature that must attack.
fn w8a_required(result: &Result<Vec<Event>, ScriptError>) -> bool {
    matches!(
        result,
        Err(ScriptError::Engine(EngineError::Illegal(
            IllegalAnswer::Required(_)
        )))
    )
}

/// Answers passively until `seat` has a decision of the kind called `kind`.
fn w8a_until(driver: &mut Driver, seat: usize, kind: &str) -> W8aResult {
    for _ in 0..300 {
        let decision = driver.decision()?.clone();
        if decision.player == pid(seat) && decision.kind.name() == kind {
            return Ok(());
        }
        driver.answer(Driver::passive_answer(&decision))?;
    }
    Err(ScriptError::Stuck(kind.into()))
}

#[rstest]
fn w8a_silent_arbiter_lets_one_attack_and_one_block(#[values(2, 3, 4)] seats: usize) -> W8aResult {
    let scenario = Scenario::new(seats)
        .at(2, 0, Step::Main1)
        .battlefield(
            0,
            [
                ready(card("Silent Arbiter").unwrap()),
                ready(free_creature("Raider", 3, 3)),
                ready(free_creature("Wolf", 2, 2)),
            ],
        )
        .battlefield(
            seats - 1,
            [
                ready(free_creature("Bear", 2, 2)),
                ready(free_creature("Guard", 2, 2)),
            ],
        );
    let setup = scenario.clone().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario)?;
    w8a_until(&mut driver, 0, "declare-attackers")?;
    let DecisionKind::DeclareAttackers(options) = &driver.decision()?.kind else {
        panic!("not declaring attackers");
    };
    assert_eq!(options.most, Some(1));
    let log = driver.engine().events().len();
    let defender = seats - 1;
    assert!(w8a_too_many(
        &driver.attack(&[("Raider", defender), ("Wolf", defender)]),
        1
    ));
    assert_eq!(
        driver.engine().events().len(),
        log,
        "a refused attack left a trace"
    );
    driver.attack(&[("Raider", defender)])?;
    w8a_until(&mut driver, defender, "declare-blockers")?;
    let DecisionKind::DeclareBlockers(options) = &driver.decision()?.kind else {
        panic!("not declaring blockers");
    };
    assert_eq!(options.most, Some(1));
    assert!(w8a_too_many(
        &driver.block(&[("Bear", "Raider"), ("Guard", "Raider")]),
        1
    ));
    driver.block(&[("Bear", "Raider")])?;
    driver.idle_until(2, Step::Main2)?;
    replays(&driver, &setup);
    Ok(())
}

#[rstest]
fn w8a_caverns_let_two_of_the_creatures_that_must_attack(
    #[values(2, 3, 4)] seats: usize,
) -> W8aResult {
    let scenario = Scenario::new(seats).at(2, 0, Step::Main1).battlefield(
        0,
        [
            ready(card("Caverns of Despair").unwrap()),
            ready(w8a_berserker("Ash")),
            ready(w8a_berserker("Birch")),
            ready(w8a_berserker("Cedar")),
            ready(free_creature("Fox", 1, 1)),
        ],
    );
    let setup = scenario.clone().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario)?;
    w8a_until(&mut driver, 0, "declare-attackers")?;
    let DecisionKind::DeclareAttackers(options) = driver.decision()?.kind.clone() else {
        panic!("not declaring attackers");
    };
    assert_eq!((options.required.len(), options.most), (3, Some(2)));
    assert_eq!(options.required_attack().len(), 2);
    assert!(w8a_required(&driver.attack(&[("Ash", 1)])));
    assert!(w8a_required(&driver.attack(&[("Ash", 1), ("Fox", 1)])));
    assert!(w8a_too_many(
        &driver.attack(&[("Ash", 1), ("Birch", 1), ("Cedar", 1)]),
        2
    ));
    driver.attack(&[("Birch", 1), ("Cedar", 1)])?;
    driver.idle_until(2, Step::Main2)?;
    let life = driver.state().player(pid(1)).unwrap().life();
    assert_eq!(life, 16, "two 2/2s connect");
    replays(&driver, &setup);
    Ok(())
}

/// The most creatures a declaration in `events` names.
fn w8a_largest(events: &[Event]) -> usize {
    events
        .iter()
        .map(|event| match event {
            Event::AttackersDeclared { attackers, .. } => attackers.len(),
            Event::BlockersDeclared { blocks, .. } => blocks.len(),
            _ => 0,
        })
        .max()
        .unwrap_or(0)
}

#[rstest]
fn w8a_bots_under_dueling_grounds_declare_one_creature_at_most(
    #[values(2, 3, 4)] seats: usize,
    #[values(1, 2, 3)] seed: u64,
) -> W8aResult {
    let mut scenario = Scenario::new(seats).seed(seed).at(2, 0, Step::Main1);
    for seat in 0..seats {
        let grounds = (seat == 0).then(|| ready(card("Dueling Grounds").unwrap()));
        scenario = scenario.battlefield(
            seat,
            grounds.into_iter().chain([
                ready(w8a_berserker("Ash")),
                ready(free_creature("Bear", 2, 2)),
                ready(free_creature("Ox", 1, 4)),
            ]),
        );
    }
    let setup = scenario.clone().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario)?;
    let mut bots: Vec<RandomBot> = (0..seats as u64)
        .map(|seat| RandomBot::new(seed, seat))
        .collect();
    let mut declared = 0;
    for _ in 0..1500 {
        if driver.state().turn().number() > 8 {
            break;
        }
        let Ok(decision) = driver.decision().cloned() else {
            break;
        };
        let answer = bots[decision.player.index()].choose(&decision);
        let events = driver.answer(answer)?;
        assert!(w8a_largest(&events) <= 1, "seed {seed}: {events:?}");
        declared += events
            .iter()
            .filter(|event| matches!(event, Event::AttackersDeclared { attackers, .. } if !attackers.is_empty()))
            .count();
    }
    assert!(declared > 0, "no creature ever attacked");
    replays(&driver, &setup);
    Ok(())
}
