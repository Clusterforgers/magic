//! The public driver: advance until a player must decide, then validate and apply answers.

use mtg_core::ids::{EffectId, ObjectId, PlayerId};

use crate::answers::{Outcome, handle};
use crate::cast::pay_options;
use crate::decision::{Answer, Decision, DecisionKind};
use crate::effects::{ContinuousEffect, Duration, Modification, Selector};
use crate::error::{EngineError, PreviewError};
use crate::events::Event;
use crate::flow::{Progress, next};
use crate::fold::apply_event;
use crate::forecast::CombatForecast;
use crate::player::LossReason;
use crate::preview::preview_forecast;
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

/// Where an engine stood before a step; [`Engine::restore`] returns it to that point.
#[derive(Debug, Clone)]
pub struct Checkpoint {
    mark: usize,
    decision: Option<Decision>,
    pending: Option<PendingCast>,
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
        let decision = self.decision.as_ref().ok_or(EngineError::NoDecision)?;
        let outcome = handle(&self.state, decision, answer)?;
        let player = decision.player;
        let checkpoint = self.checkpoint();
        let result = self.run(outcome, player).and_then(|()| self.advance());
        self.finish(checkpoint, result)
    }

    /// What combat damage would do if `seat` gave `answer` to the decision it faces now, without giving it.
    ///
    /// Only a pending attack or block can be previewed. The answer is validated exactly as [`apply`](Engine::apply)
    /// would, so an illegal one is an error and never a forecast. The engine is not touched: no event is logged,
    /// no random number is drawn and the state hash stays as it is.
    pub fn preview_forecast(
        &self,
        seat: PlayerId,
        answer: &Answer,
    ) -> Result<CombatForecast, PreviewError> {
        let decision = self.decision.as_ref().ok_or(PreviewError::NoDecision)?;
        if decision.player != seat {
            return Err(PreviewError::NotYourDecision(seat));
        }
        preview_forecast(&self.state, decision, answer)
    }

    /// Removes `player` from the game at once (CR 104.3a); a player may concede at any time.
    ///
    /// Returns the events produced: the loss, what follows from it (their objects leaving, the end
    /// of the turn if it was theirs, the end of the game if one player remains) and the advance to
    /// the next decision. A cast the engine was waiting to see paid for is abandoned. On error the
    /// engine is untouched.
    pub fn concede(&mut self, player: PlayerId) -> Result<Vec<Event>, EngineError> {
        if self.decision.is_none() {
            return Err(EngineError::NoDecision);
        }
        if !self.state.is_alive(player) {
            return Err(EngineError::NotInGame(player));
        }
        let checkpoint = self.checkpoint();
        self.pending = None;
        let lost = Event::PlayerLost {
            player,
            reason: LossReason::Conceded,
        };
        let result = self.commit(vec![lost]).and_then(|()| self.advance());
        self.finish(checkpoint, result)
    }

    /// Notes the current position so that the steps taken after it can be undone with [`restore`](Engine::restore).
    pub fn checkpoint(&self) -> Checkpoint {
        Checkpoint {
            mark: self.events.len(),
            decision: self.decision.clone(),
            pending: self.pending,
        }
    }

    /// Undoes every step taken since `checkpoint`: the log is cut back and the state rebuilt from it.
    ///
    /// The checkpoint must come from this engine and not lie beyond the current log. Callers use
    /// this when persisting a step fails, so the in-memory game never runs ahead of the store.
    pub fn restore(&mut self, checkpoint: Checkpoint) -> Result<(), EngineError> {
        if checkpoint.mark > self.events.len() {
            return Err(EngineError::InvalidEvent(format!(
                "checkpoint at event {} lies beyond the log of {} events",
                checkpoint.mark,
                self.events.len()
            )));
        }
        let state = replay(&self.setup, &self.events[..checkpoint.mark])?;
        self.events.truncate(checkpoint.mark);
        self.state = state;
        self.decision = checkpoint.decision;
        self.pending = checkpoint.pending;
        Ok(())
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
        let checkpoint = self.checkpoint();
        self.pending = None;
        let result = self
            .commit(vec![Event::EffectCreated { effect }])
            .and_then(|()| self.advance());
        self.finish(checkpoint, result)
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

    /// The events since `checkpoint` on success; on failure the step is undone so it never half-applies.
    fn finish(
        &mut self,
        checkpoint: Checkpoint,
        result: Result<(), EngineError>,
    ) -> Result<Vec<Event>, EngineError> {
        let mark = checkpoint.mark;
        match result {
            Ok(()) => Ok(self.events[mark..].to_vec()),
            Err(error) => {
                // A prefix that already folded once cannot fail to replay; if it somehow does, the original error is the useful one.
                let _ = self.restore(checkpoint);
                Err(error)
            }
        }
    }
}
