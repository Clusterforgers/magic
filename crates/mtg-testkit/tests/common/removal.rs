//! Goal: shared helpers for the tests of destroying, exiling and countering: tables of seats with creatures on them, passing to a seat, the moves of objects a log describes, and a scripted game whose log is rebuilt and replayed (this is not a test module itself).
#![allow(dead_code, clippy::unwrap_used, clippy::expect_used)]

use std::collections::VecDeque;
use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::keyword::Keyword;
use mtg_core::color::Color;
use mtg_engine::choose::{Target, TargetChoice};
use mtg_engine::decision::{Answer, DecisionKind, PayChoice, PriorityAction};
use mtg_engine::events::ZoneCause;
use mtg_engine::zones::Zone;
use mtg_engine::{Decision, Engine, EngineError, Event, GameSetup, GameState, replay};
use mtg_testkit::cards::{creature, free_creature, keyworded};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};

use super::{name_of, pid};

/// The corpus card called `name`.
pub fn corpus(name: &str) -> Arc<CardDefinition> {
    card(name).unwrap()
}

/// A 5/5 that cannot be destroyed.
pub fn titan() -> Arc<CardDefinition> {
    keyworded("Titan", 5, 5, &[Keyword::Indestructible])
}

/// A black 2/2, which Doom Blade cannot target.
pub fn gravedigger() -> Arc<CardDefinition> {
    creature("Gravedigger", 0, &[Color::Black], 2, 2)
}

/// The creatures of each seat: seat 0 a Wolf, seat 1 Grizzly Bears and the black Gravedigger, seat 2 Serra Angel and
/// seat 3 Llanowar Elves.
pub fn creatures(seats: usize) -> Vec<Vec<Arc<CardDefinition>>> {
    let all = [
        vec![corpus("Grizzly Bears"), gravedigger()],
        vec![corpus("Serra Angel")],
        vec![corpus("Llanowar Elves")],
    ];
    std::iter::once(vec![free_creature("Wolf", 3, 3)])
        .chain(all.into_iter().take(seats - 1))
        .collect()
}

/// A table where seat 0 holds `hand` and controls `lands`, every seat controls its [`creatures`], and each of
/// `extra` joins the battlefield of its seat.
pub fn table(
    seats: usize,
    hand: &[&str],
    lands: &[Arc<CardDefinition>],
    extra: &[(usize, Arc<CardDefinition>)],
) -> Scenario {
    let mut scenario = Scenario::new(seats).hand(0, hand.iter().map(|name| corpus(name)));
    for (seat, board) in creatures(seats).into_iter().enumerate() {
        let mut permanents: Vec<_> = board.into_iter().map(ready).collect();
        if seat == 0 {
            permanents.extend(lands.iter().cloned().map(ready));
        }
        permanents.extend(
            extra
                .iter()
                .filter(|(at, _)| *at == seat)
                .map(|(_, card)| ready(card.clone())),
        );
        scenario = scenario.battlefield(seat, permanents);
    }
    scenario
}

/// Passes priority until the stack is empty.
pub fn resolve_the_stack(driver: &mut Driver) -> Result<(), ScriptError> {
    while !driver.state().stack().is_empty() {
        driver.pass()?;
    }
    Ok(())
}

/// Passes priority until `seat` holds it.
pub fn pass_to(driver: &mut Driver, seat: usize) -> Result<(), ScriptError> {
    for _ in 0..16 {
        if driver.decision()?.player == pid(seat) {
            return Ok(());
        }
        driver.pass()?;
    }
    panic!("seat {seat} never received priority");
}

/// The spell on the stack called `name`, as a target.
pub fn spell_named(driver: &Driver, name: &str) -> Target {
    let state = driver.state();
    let id = state
        .stack()
        .iter()
        .copied()
        .find(|id| name_of(state, *id) == name)
        .unwrap();
    Target::Object(state.object(id).unwrap().reference())
}

/// The moves of objects between zones that the events since `from` describe: (name, from, to, cause).
pub fn moves(driver: &Driver, from: usize) -> Vec<(String, Zone, Zone, ZoneCause)> {
    driver.engine().events()[from..]
        .iter()
        .filter_map(|event| match event {
            Event::ZoneChanged {
                object,
                from,
                to,
                cause,
            } => Some((name_of(driver.state(), *object), *from, *to, *cause)),
            _ => None,
        })
        .collect()
}

/// The seat that owns the battlefield permanent called `name`.
pub fn owner_of(driver: &Driver, name: &str) -> usize {
    let id = driver.id_of(name).unwrap();
    driver.state().object(id).unwrap().owner().index()
}

/// The names, sorted.
pub fn sorted(mut names: Vec<String>) -> Vec<String> {
    names.sort();
    names
}

/// Owned copies of these names.
pub fn strings(names: &[&str]) -> Vec<String> {
    names.iter().map(|name| (*name).to_owned()).collect()
}

/// What a scripted cast is aimed at.
#[derive(Debug, Clone)]
pub enum Aim {
    /// The spell has no target.
    Nothing,
    /// The creature on the battlefield with this name.
    Creature(&'static str),
    /// The spell on the stack with this name.
    Spell(&'static str),
}

/// One cast of a script: `seat` casts `card` the first time it holds priority with the card castable.
#[derive(Debug, Clone)]
pub struct Step {
    /// Who casts it.
    pub seat: usize,
    /// The card.
    pub card: &'static str,
    /// What it is aimed at.
    pub aim: Aim,
}

/// A scripted game: the setup, the whole log, the (log length, state hash) at every decision and where each
/// decision's question stood.
pub struct Played {
    /// What the game started from.
    pub setup: GameSetup,
    /// Every event.
    pub log: Vec<Event>,
    /// The log length, the state hash and the decision pending at each point where a seat was asked, the first one
    /// and the end included.
    pub points: Vec<(usize, u64, Option<Decision>)>,
}

fn answer_for(
    state: &GameState,
    decision: &Decision,
    steps: &mut VecDeque<Step>,
    current: &mut Option<Step>,
) -> Answer {
    match &decision.kind {
        DecisionKind::Priority(options) => {
            if let Some(step) = steps.front()
                && decision.player == pid(step.seat)
                && let Some(card) = options
                    .castable
                    .iter()
                    .find(|id| name_of(state, **id) == step.card)
            {
                *current = steps.pop_front();
                return Answer::Priority(PriorityAction::CastSpell(*card));
            }
            Answer::Priority(PriorityAction::Pass)
        }
        DecisionKind::ChooseTargets(options) => {
            let aim = current.as_ref().map(|step| step.aim.clone()).unwrap();
            let chosen = options
                .legal
                .iter()
                .copied()
                .find(|target| match (&aim, target) {
                    (Aim::Creature(name) | Aim::Spell(name), Target::Object(object)) => {
                        name_of(state, object.id) == *name
                    }
                    _ => false,
                })
                .unwrap_or_else(|| panic!("{aim:?} is not among {:?}", options.legal));
            Answer::ChooseTargets(TargetChoice::Targets(vec![chosen]))
        }
        DecisionKind::PayMana(_) => Answer::PayMana(PayChoice::Auto),
        other => panic!("the script does not expect {}", other.name()),
    }
}

/// Plays the casts of `steps` in order from the first decision of `setup`, every other seat passing, until the steps are
/// cast and the stack is empty.
pub fn play_script(setup: GameSetup, steps: Vec<Step>) -> Result<Played, EngineError> {
    let mut engine = Engine::new(setup.clone())?;
    let mut steps: VecDeque<Step> = steps.into();
    let mut current = None;
    let mut points = vec![(
        engine.events().len(),
        engine.state_hash(),
        engine.decision().cloned(),
    )];
    for _ in 0..600 {
        let Some(decision) = engine.decision().cloned() else {
            break;
        };
        let answer = answer_for(engine.state(), &decision, &mut steps, &mut current);
        engine.apply(answer)?;
        points.push((
            engine.events().len(),
            engine.state_hash(),
            engine.decision().cloned(),
        ));
        if steps.is_empty() && engine.state().stack().is_empty() && current.is_some() {
            let casting = matches!(
                engine.decision().map(|d| &d.kind),
                Some(DecisionKind::ChooseTargets(_) | DecisionKind::PayMana(_))
            );
            if !casting {
                break;
            }
        }
    }
    assert!(steps.is_empty(), "casts left over: {steps:?}");
    Ok(Played {
        setup,
        log: engine.events().to_vec(),
        points,
    })
}

/// Checks the log of a scripted game three ways: it replays to the live state at every point, an engine rebuilt at
/// every decision stands where the original stood (a half-finished cast, which lives outside the state, asks for
/// priority again), and an engine rebuilt at the start of a resolution and after each of its steps goes on to the next
/// decision with the log and the state the original had.
pub fn check_rebuilds(played: &Played) -> Result<(), EngineError> {
    let Played { setup, log, points } = played;
    for (len, hash, _) in points {
        assert_eq!(
            replay(setup, &log[..*len])?.state_hash(),
            *hash,
            "replay of {len} events"
        );
    }
    for (len, hash, decision) in points {
        let resumed = Engine::resume(setup.clone(), log[..*len].to_vec())?;
        assert_eq!(resumed.state_hash(), *hash, "rebuilt at {len}");
        let mid_cast = match decision.as_ref().map(|d| &d.kind) {
            Some(DecisionKind::PayMana(_)) => true,
            Some(DecisionKind::ChooseTargets(options)) => !options.triggered,
            _ => false,
        };
        if mid_cast {
            let again = resumed.decision().map(|d| (d.player, d.kind.name()));
            let asked = decision.as_ref().map(|d| d.player);
            assert_eq!(
                again.map(|(player, _)| Some(player)),
                Some(asked),
                "at {len}"
            );
            assert_eq!(again.map(|(_, kind)| kind), Some("priority"), "at {len}");
        } else {
            assert_eq!(resumed.decision(), decision.as_ref(), "rebuilt at {len}");
        }
    }
    // A log is cut where a batch of events and the triggers it set off are both complete: the engine finds triggers as
    // it commits a batch, so a cut between the two would lose them.
    let boundaries: Vec<usize> = log
        .iter()
        .enumerate()
        .filter(|(_, e)| {
            matches!(
                e,
                Event::ResolutionBegan { .. } | Event::StepDone { .. } | Event::AbilityTriggered(_)
            )
        })
        .map(|(at, _)| at + 1)
        .filter(|cut| !matches!(log.get(*cut), Some(Event::AbilityTriggered(_))))
        .collect();
    assert!(!boundaries.is_empty(), "a spell resolved");
    for cut in boundaries {
        let resumed = Engine::resume(setup.clone(), log[..cut].to_vec())?;
        let (len, hash, _) = points.iter().find(|(len, ..)| *len >= cut).unwrap();
        assert_eq!(resumed.events(), &log[..*len], "cut at {cut}");
        assert_eq!(resumed.state_hash(), *hash, "cut at {cut}");
    }
    Ok(())
}
