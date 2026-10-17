//! A scripted-answer driver: feed answers by card name, then assert on state and events.

use mtg_core::ids::{ObjectId, PlayerId};
use mtg_engine::combat::AttackTarget;
use mtg_engine::decision::{
    Answer, DecisionKind, MulliganChoice, PayChoice, PriorityAction, PriorityOptions,
};
use mtg_engine::mana::{ManaSource, Payment};
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_engine::{Decision, Engine, EngineError, Event, GameState};
use thiserror::Error;

use crate::invariants::{InvariantChecker, Violation};
use crate::scenario::{Scenario, ScenarioError};

/// Safety valve for loops that wait for a step to arrive.
const MAX_WAIT_ANSWERS: usize = 20_000;

/// A scripted step failed.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ScriptError {
    /// The engine rejected an answer.
    #[error("engine: {0}")]
    Engine(#[from] EngineError),
    /// An invariant broke after an answer.
    #[error("invariant: {0}")]
    Invariant(#[from] Violation),
    /// The scenario could not be built.
    #[error("scenario: {0}")]
    Scenario(#[from] ScenarioError),
    /// The script named a card that is not among the options.
    #[error("no option named {name:?} among {place}")]
    NoSuchCard {
        /// The requested card name.
        name: String,
        /// Where it was looked for.
        place: &'static str,
    },
    /// The pending decision is not the kind the script step needs.
    #[error("expected a {wanted} decision but the engine asked for {got}")]
    UnexpectedDecision {
        /// What the step needed.
        wanted: &'static str,
        /// What is pending.
        got: String,
    },
    /// An `expect_*` assertion did not hold.
    #[error("expectation failed: {0}")]
    Expectation(String),
    /// The script waited too long for a step.
    #[error("gave up waiting for {0}")]
    Stuck(String),
}

/// Drives an [`Engine`] with scripted answers and checks invariants after every answer.
#[derive(Debug)]
pub struct Driver {
    engine: Engine,
    checker: InvariantChecker,
}

impl Driver {
    /// Wraps an engine that has just been created.
    pub fn new(engine: Engine) -> Result<Self, ScriptError> {
        let mut checker = InvariantChecker::new(engine.setup())?;
        checker.observe(engine.state(), engine.events())?;
        Ok(Driver { engine, checker })
    }

    /// Builds a scenario and wraps the resulting engine.
    pub fn from_scenario(scenario: Scenario) -> Result<Self, ScriptError> {
        Driver::new(scenario.build()?)
    }

    /// The engine.
    pub fn engine(&self) -> &Engine {
        &self.engine
    }

    /// The current state.
    pub fn state(&self) -> &GameState {
        self.engine.state()
    }

    /// The pending decision.
    pub fn decision(&self) -> Result<&Decision, ScriptError> {
        self.engine
            .decision()
            .ok_or(ScriptError::Engine(EngineError::NoDecision))
    }

    /// Applies a raw answer and checks invariants; returns the events it produced.
    pub fn answer(&mut self, answer: Answer) -> Result<Vec<Event>, ScriptError> {
        let events = self.engine.apply(answer)?;
        self.checker
            .observe(self.engine.state(), self.engine.events())?;
        Ok(events)
    }

    /// Applies several raw answers in order.
    pub fn answers(
        &mut self,
        answers: impl IntoIterator<Item = Answer>,
    ) -> Result<(), ScriptError> {
        for answer in answers {
            self.answer(answer)?;
        }
        Ok(())
    }

    fn name_of(&self, id: ObjectId) -> &str {
        self.state()
            .object(id)
            .map_or("?", |o| o.card().name.as_str())
    }

    fn priority(&self) -> Result<(PlayerId, &PriorityOptions), ScriptError> {
        let decision = self.decision()?;
        match &decision.kind {
            DecisionKind::Priority(options) => Ok((decision.player, options)),
            other => Err(ScriptError::UnexpectedDecision {
                wanted: "priority",
                got: other.name().to_owned(),
            }),
        }
    }

    fn find(
        &self,
        ids: impl IntoIterator<Item = ObjectId>,
        name: &str,
        place: &'static str,
    ) -> Result<ObjectId, ScriptError> {
        ids.into_iter()
            .find(|id| self.name_of(*id) == name)
            .ok_or_else(|| ScriptError::NoSuchCard {
                name: name.to_owned(),
                place,
            })
    }

    /// The player holding priority passes.
    pub fn pass(&mut self) -> Result<Vec<Event>, ScriptError> {
        self.priority()?;
        self.answer(Answer::Priority(PriorityAction::Pass))
    }

    /// The player holding priority plays the named land from hand.
    pub fn play_land(&mut self, name: &str) -> Result<Vec<Event>, ScriptError> {
        let (_, options) = self.priority()?;
        let id = self.find(options.playable_lands.clone(), name, "playable lands")?;
        self.answer(Answer::Priority(PriorityAction::PlayLand(id)))
    }

    /// The player holding priority taps the named permanent for mana.
    pub fn tap_for_mana(&mut self, name: &str) -> Result<Vec<Event>, ScriptError> {
        let (_, options) = self.priority()?;
        let source = options
            .mana_abilities
            .iter()
            .find(|m| self.name_of(m.source.object) == name)
            .map(|m| m.source)
            .ok_or_else(|| ScriptError::NoSuchCard {
                name: name.to_owned(),
                place: "mana abilities",
            })?;
        self.answer(Answer::Priority(PriorityAction::ActivateMana(source)))
    }

    /// Starts casting the named spell and pays automatically.
    pub fn cast(&mut self, name: &str) -> Result<Vec<Event>, ScriptError> {
        self.begin_cast(name)?;
        self.answer(Answer::PayMana(PayChoice::Auto))
    }

    /// Starts casting the named spell and pays by tapping the named lands.
    pub fn cast_tapping(&mut self, name: &str, lands: &[&str]) -> Result<Vec<Event>, ScriptError> {
        self.begin_cast(name)?;
        let DecisionKind::PayMana(options) = &self.decision()?.kind else {
            return Err(self.unexpected("pay-mana"));
        };
        let mut taps: Vec<ManaSource> = Vec::new();
        for land in lands {
            let free = options
                .sources
                .iter()
                .map(|o| o.source)
                .filter(|s| !taps.contains(s))
                .map(|s| s.object);
            let object = self.find(free, land, "payment sources")?;
            let source = options
                .sources
                .iter()
                .find(|o| o.source.object == object)
                .map(|o| o.source)
                .ok_or_else(|| ScriptError::NoSuchCard {
                    name: (*land).to_owned(),
                    place: "payment sources",
                })?;
            taps.push(source);
        }
        self.answer(Answer::PayMana(PayChoice::Manual(Payment {
            pool: Vec::new(),
            taps,
        })))
    }

    /// Chooses the spell to cast; the payment decision is then pending.
    pub fn begin_cast(&mut self, name: &str) -> Result<(), ScriptError> {
        let (_, options) = self.priority()?;
        let id = self.find(options.castable.clone(), name, "castable spells")?;
        self.answer(Answer::Priority(PriorityAction::CastSpell(id)))?;
        Ok(())
    }

    fn unexpected(&self, wanted: &'static str) -> ScriptError {
        ScriptError::UnexpectedDecision {
            wanted,
            got: self
                .engine
                .decision()
                .map_or_else(|| "nothing".to_owned(), |d| d.kind.name().to_owned()),
        }
    }

    /// The deciding player attacks: each pair is (attacker name, defending player index).
    pub fn attack(&mut self, attacks: &[(&str, usize)]) -> Result<Vec<Event>, ScriptError> {
        let DecisionKind::DeclareAttackers(options) = &self.decision()?.kind else {
            return Err(self.unexpected("declare-attackers"));
        };
        let mut chosen: Vec<(ObjectId, AttackTarget)> = Vec::new();
        for (name, defender) in attacks {
            let free = options
                .attackers
                .iter()
                .map(|a| a.object)
                .filter(|o| !chosen.iter().any(|(c, _)| c == o));
            let object = self.find(free, name, "legal attackers")?;
            chosen.push((
                object,
                AttackTarget::Player(PlayerId::from_index(*defender)),
            ));
        }
        self.answer(Answer::DeclareAttackers(chosen))
    }

    /// The deciding player blocks: each pair is (blocker name, attacker name).
    pub fn block(&mut self, blocks: &[(&str, &str)]) -> Result<Vec<Event>, ScriptError> {
        let DecisionKind::DeclareBlockers(options) = &self.decision()?.kind else {
            return Err(self.unexpected("declare-blockers"));
        };
        let mut chosen: Vec<(ObjectId, ObjectId)> = Vec::new();
        for (blocker, attacker) in blocks {
            let free = options
                .blockers
                .iter()
                .map(|b| b.object)
                .filter(|o| !chosen.iter().any(|(c, _)| c == o));
            let blocker_id = self.find(free, blocker, "legal blockers")?;
            let can_block = options
                .blockers
                .iter()
                .find(|b| b.object == blocker_id)
                .map(|b| b.can_block.clone())
                .unwrap_or_default();
            let attacker_id = self.find(can_block, attacker, "attackers the blocker can block")?;
            chosen.push((blocker_id, attacker_id));
        }
        self.answer(Answer::DeclareBlockers(chosen))
    }

    /// The attacker's controller divides damage: each pair is (blocker name, damage).
    pub fn assign_damage(&mut self, shares: &[(&str, u32)]) -> Result<Vec<Event>, ScriptError> {
        let DecisionKind::AssignDamage(options) = &self.decision()?.kind else {
            return Err(self.unexpected("assign-damage"));
        };
        let mut assignment = Vec::new();
        for (name, amount) in shares {
            let free = options
                .blockers
                .iter()
                .copied()
                .filter(|b| !assignment.iter().any(|(c, _)| c == b));
            assignment.push((self.find(free, name, "blockers")?, *amount));
        }
        self.answer(Answer::AssignDamage(assignment))
    }

    /// A passive answer to whatever is pending: pass, keep, attack with nothing, block nothing.
    fn passive_answer(decision: &Decision) -> Answer {
        match &decision.kind {
            DecisionKind::Mulligan(_) => Answer::Mulligan(MulliganChoice::Keep),
            DecisionKind::BottomCards(o) => {
                Answer::BottomCards(o.hand.iter().copied().take(o.count).collect())
            }
            DecisionKind::Priority(_) => Answer::Priority(PriorityAction::Pass),
            DecisionKind::PayMana(_) => Answer::PayMana(PayChoice::Cancel),
            DecisionKind::DeclareAttackers(_) => Answer::DeclareAttackers(Vec::new()),
            DecisionKind::DeclareBlockers(_) => Answer::DeclareBlockers(Vec::new()),
            DecisionKind::AssignDamage(o) => Answer::AssignDamage(
                o.blockers
                    .iter()
                    .enumerate()
                    .map(|(i, b)| (*b, if i == 0 { o.power } else { 0 }))
                    .collect(),
            ),
            DecisionKind::Discard(o) => {
                Answer::Discard(o.hand.iter().copied().take(o.count).collect())
            }
        }
    }

    /// Answers passively until the game reaches `step` of turn `turn` (a decision is pending there).
    pub fn idle_until(&mut self, turn: u32, step: Step) -> Result<(), ScriptError> {
        for _ in 0..MAX_WAIT_ANSWERS {
            let state = self.state();
            if state.turn().number() == turn
                && state.turn().step() == step
                && self.engine.decision().is_some()
            {
                return Ok(());
            }
            let decision = self.decision()?.clone();
            self.answer(Driver::passive_answer(&decision))?;
        }
        Err(ScriptError::Stuck(format!("turn {turn} {step:?}")))
    }

    /// Passes priority (only) until the current turn reaches `step`; any other decision is an error.
    pub fn pass_until(&mut self, step: Step) -> Result<(), ScriptError> {
        for _ in 0..MAX_WAIT_ANSWERS {
            if self.state().turn().step() == step && self.engine.decision().is_some() {
                return Ok(());
            }
            self.pass()?;
        }
        Err(ScriptError::Stuck(format!("{step:?}")))
    }

    /// Asserts a player's life total.
    pub fn expect_life(&self, player: usize, life: i32) -> Result<(), ScriptError> {
        let actual = self
            .state()
            .player(PlayerId::from_index(player))
            .map(|p| p.life());
        expectation(actual == Some(life), || {
            format!("player {player} life is {actual:?}, expected {life}")
        })
    }

    /// Asserts the names in a zone, ignoring order.
    pub fn expect_zone(&self, zone: Zone, names: &[&str]) -> Result<(), ScriptError> {
        let mut actual: Vec<&str> = self
            .state()
            .zone(zone)
            .iter()
            .map(|id| self.name_of(*id))
            .collect();
        let mut expected = names.to_vec();
        actual.sort_unstable();
        expected.sort_unstable();
        expectation(actual == expected, || {
            format!("{zone:?} holds {actual:?}, expected {expected:?}")
        })
    }

    /// Asserts the current step.
    pub fn expect_step(&self, step: Step) -> Result<(), ScriptError> {
        let actual = self.state().turn().step();
        expectation(actual == step, || {
            format!("step is {actual:?}, expected {step:?}")
        })
    }

    /// Asserts a decision of `kind` is pending for `player`.
    pub fn expect_decision(&self, player: usize, kind: &str) -> Result<(), ScriptError> {
        let decision = self.decision()?;
        expectation(
            decision.player == PlayerId::from_index(player) && decision.kind.name() == kind,
            || {
                format!(
                    "pending {} for {:?}, expected {kind} for player {player}",
                    decision.kind.name(),
                    decision.player
                )
            },
        )
    }

    /// Asserts some event in the whole log satisfies `matches`.
    pub fn expect_event(
        &self,
        what: &str,
        matches: impl Fn(&Event) -> bool,
    ) -> Result<(), ScriptError> {
        expectation(self.engine.events().iter().any(matches), || {
            format!("no event matching {what}")
        })
    }

    /// Asserts no event in the whole log satisfies `matches`.
    pub fn expect_no_event(
        &self,
        what: &str,
        matches: impl Fn(&Event) -> bool,
    ) -> Result<(), ScriptError> {
        expectation(!self.engine.events().iter().any(matches), || {
            format!("unexpected event matching {what}")
        })
    }
}

fn expectation(holds: bool, message: impl FnOnce() -> String) -> Result<(), ScriptError> {
    if holds {
        Ok(())
    } else {
        Err(ScriptError::Expectation(message()))
    }
}
