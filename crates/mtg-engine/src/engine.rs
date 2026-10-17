//! The public driver: advance until a player must decide, then validate and apply answers.

use mtg_core::ids::{EffectId, ObjectId, PlayerId};

use crate::answers::{Outcome, handle};
use crate::cast::pay_options;
use crate::decision::{Answer, Decision, DecisionKind};
use crate::effects::{ContinuousEffect, Duration, Modification, Selector};
use crate::error::EngineError;
use crate::events::Event;
use crate::flow::{Progress, next};
use crate::fold::apply_event;
use crate::setup::GameSetup;
use crate::state::{GameResult, GameState};

/// Internal progress steps allowed between decisions before the engine reports a stall.
const MAX_INTERNAL_STEPS: usize = 10_000;

/// A spell whose payment has not been chosen yet; it lives outside game state until paid.
#[derive(Debug, Clone, Copy)]
struct PendingCast {
    player: PlayerId,
    spell: ObjectId,
}

/// Replays `events` over the initial state of `setup`.
pub fn replay(setup: &GameSetup, events: &[Event]) -> Result<GameState, EngineError> {
    let mut state = GameState::from_setup(setup)?;
    for event in events {
        apply_event(&mut state, event)?;
    }
    Ok(state)
}

/// A running game: state, the event log that produced it, and the decision it is waiting on.
#[derive(Debug)]
pub struct Engine {
    setup: GameSetup,
    state: GameState,
    events: Vec<Event>,
    decision: Option<Decision>,
    pending: Option<PendingCast>,
}

impl Engine {
    /// Starts a game and runs it to the first decision.
    pub fn new(setup: GameSetup) -> Result<Engine, EngineError> {
        Engine::resume(setup, Vec::new())
    }

    /// Rebuilds a game from its setup and event log, then runs to the next decision.
    pub fn resume(setup: GameSetup, events: Vec<Event>) -> Result<Engine, EngineError> {
        let state = replay(&setup, &events)?;
        let mut engine = Engine {
            setup,
            state,
            events,
            decision: None,
            pending: None,
        };
        engine.advance()?;
        Ok(engine)
    }

    /// The current state.
    pub fn state(&self) -> &GameState {
        &self.state
    }

    /// Fingerprint of the current state.
    pub fn state_hash(&self) -> u64 {
        self.state.state_hash()
    }

    /// Every event so far, from the start of the game.
    pub fn events(&self) -> &[Event] {
        &self.events
    }

    /// The setup the game was created from.
    pub fn setup(&self) -> &GameSetup {
        &self.setup
    }

    /// The pending decision; `None` once the game is over.
    pub fn decision(&self) -> Option<&Decision> {
        self.decision.as_ref()
    }

    /// The result once the game has ended.
    pub fn result(&self) -> Option<GameResult> {
        self.state.result()
    }

    /// Validates `answer` against the pending decision and advances to the next one.
    ///
    /// Illegal answers return an error and leave the engine untouched. On success returns every
    /// event produced, including the automatic steps that followed the answer.
    pub fn apply(&mut self, answer: Answer) -> Result<Vec<Event>, EngineError> {
        let decision = self.decision.clone().ok_or(EngineError::NoDecision)?;
        let outcome = handle(&decision, answer)?;
        let mark = self.events.len();
        let previous = self.pending;
        let result = self
            .run(outcome, decision.player)
            .and_then(|()| self.advance());
        match result {
            Ok(()) => Ok(self.events[mark..].to_vec()),
            Err(error) => {
                self.rollback(mark, decision, previous);
                Err(error)
            }
        }
    }

    /// Registers a continuous effect and logs it.
    ///
    /// This is the entry point effect sources (spells, abilities) will use; until they exist it is
    /// the hook tests use to prove buffs, debuffs, durations and layering.
    pub fn add_continuous_effect(
        &mut self,
        selector: Selector,
        modification: Modification,
        duration: Duration,
    ) -> Result<Vec<Event>, EngineError> {
        if self.decision.is_none() {
            return Err(EngineError::NoDecision);
        }
        let effect = ContinuousEffect::new(
            EffectId(self.state.next_effect),
            selector,
            modification,
            duration,
            self.state.next_timestamp,
        );
        let mark = self.events.len();
        let decision = self.decision.clone();
        let previous = self.pending;
        self.pending = None;
        let result = self
            .commit(vec![Event::EffectCreated { effect }])
            .and_then(|()| self.advance());
        match result {
            Ok(()) => Ok(self.events[mark..].to_vec()),
            Err(error) => {
                if let Some(decision) = decision {
                    self.rollback(mark, decision, previous);
                }
                Err(error)
            }
        }
    }

    fn run(&mut self, outcome: Outcome, player: PlayerId) -> Result<(), EngineError> {
        match outcome {
            Outcome::Events(events) => {
                self.pending = None;
                self.commit(events)
            }
            Outcome::BeginCast(spell) => {
                self.pending = Some(PendingCast { player, spell });
                Ok(())
            }
            Outcome::CancelCast => {
                self.pending = None;
                Ok(())
            }
        }
    }

    fn commit(&mut self, events: Vec<Event>) -> Result<(), EngineError> {
        for event in events {
            apply_event(&mut self.state, &event)?;
            self.events.push(event);
        }
        Ok(())
    }

    /// Runs the game until a decision is needed or the game ends.
    fn advance(&mut self) -> Result<(), EngineError> {
        for _ in 0..MAX_INTERNAL_STEPS {
            if let Some(cast) = self.pending {
                match pay_options(&self.state, cast.player, cast.spell) {
                    Some(options) => {
                        self.decision = Some(Decision {
                            player: cast.player,
                            kind: DecisionKind::PayMana(options),
                        });
                        return Ok(());
                    }
                    None => self.pending = None,
                }
            }
            match next(&self.state) {
                Progress::Decide(decision) => {
                    self.decision = Some(decision);
                    return Ok(());
                }
                Progress::Done => {
                    self.decision = None;
                    return Ok(());
                }
                Progress::Events(events) => self.commit(events)?,
            }
        }
        Err(EngineError::Stalled(MAX_INTERNAL_STEPS))
    }

    /// Restores the state as of event `mark` after a failed step so a failure never half-applies.
    fn rollback(&mut self, mark: usize, decision: Decision, pending: Option<PendingCast>) {
        self.events.truncate(mark);
        if let Ok(state) = replay(&self.setup, &self.events) {
            self.state = state;
        }
        self.decision = Some(decision);
        self.pending = pending;
    }
}
