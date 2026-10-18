//! A started game as a plain state machine: the engine plus the decision ids that tell stale answers from current ones.

use mtg_core::ids::PlayerId;
use mtg_engine::forecast::CombatForecast;
use mtg_engine::{Answer, Checkpoint, Decision, Engine, EngineError, Event, GameState};
use mtg_protocol::{DecisionId, ErrorCode, ServerMessage, StateView};

/// A client message that was refused, as the `Error` it becomes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Rejection {
    pub(crate) code: ErrorCode,
    pub(crate) message: String,
}

impl Rejection {
    pub(crate) fn new(code: ErrorCode, message: impl Into<String>) -> Rejection {
        Rejection {
            code,
            message: message.into(),
        }
    }

    pub(crate) fn into_message(self) -> ServerMessage {
        ServerMessage::Error {
            code: self.code,
            message: self.message,
        }
    }

    fn engine_failed() -> Rejection {
        Rejection::new(
            ErrorCode::Internal,
            "the game engine failed; the game is unchanged",
        )
    }
}

/// One engine step taken in memory but not yet persisted or announced.
#[derive(Debug)]
pub(crate) struct Step {
    /// The state before the step, which event projection needs.
    pub(crate) before: GameState,
    /// Undoes the step if it cannot be persisted.
    pub(crate) checkpoint: Checkpoint,
    /// The events the step produced.
    pub(crate) events: Vec<Event>,
    /// Log position of the first event.
    pub(crate) from_seq: u64,
    /// The seat whose message caused the step.
    pub(crate) origin: PlayerId,
    /// Whether a pending decision the step left unchanged keeps its id.
    pub(crate) keep_unchanged: bool,
}

/// The decision awaiting an answer and the id the client must echo.
#[derive(Debug)]
struct Pending {
    id: DecisionId,
    decision: Decision,
}

/// A started game.
#[derive(Debug)]
pub(crate) struct Running {
    pub(crate) engine: Engine,
    pending: Option<Pending>,
    last_id: u64,
}

pub(crate) fn count(len: usize) -> u64 {
    u64::try_from(len).unwrap_or(u64::MAX)
}

impl Running {
    /// Wraps `engine`; call [`settle`](Running::settle) to issue the first decision id.
    pub(crate) fn new(engine: Engine) -> Running {
        Running {
            engine,
            pending: None,
            last_id: 0,
        }
    }

    /// The seat that must answer, if any.
    pub(crate) fn waiting_on(&self) -> Option<PlayerId> {
        self.pending.as_ref().map(|p| p.decision.player)
    }

    /// Aligns the pending decision with the engine's and returns what a client must be sent, if anything.
    ///
    /// Every new decision gets a fresh id. With `keep_unchanged`, a decision that did not change keeps
    /// its id and is not sent again, so a concession elsewhere does not invalidate a pending answer.
    pub(crate) fn settle(&mut self, keep_unchanged: bool) -> Option<(DecisionId, Decision)> {
        let Some(decision) = self.engine.decision().cloned() else {
            self.pending = None;
            return None;
        };
        let unchanged = self
            .pending
            .as_ref()
            .is_some_and(|p| p.decision == decision);
        if keep_unchanged && unchanged {
            return None;
        }
        self.last_id += 1;
        let id = DecisionId(self.last_id);
        self.pending = Some(Pending {
            id,
            decision: decision.clone(),
        });
        Some((id, decision))
    }

    /// What a client needs to catch up: a snapshot, its pending decision and the result if the game is over.
    pub(crate) fn sync_messages(&self, seat: PlayerId) -> Vec<ServerMessage> {
        let mut messages = vec![ServerMessage::Snapshot {
            seq: count(self.engine.events().len()),
            state: Box::new(StateView::for_player(self.engine.state(), seat)),
        }];
        if let Some(pending) = self.pending.as_ref().filter(|p| p.decision.player == seat) {
            messages.push(ServerMessage::Decision {
                id: pending.id,
                decision: pending.decision.clone(),
            });
        }
        if let Some(result) = self.engine.result() {
            messages.push(ServerMessage::GameOver { result });
        }
        messages
    }

    /// Applies `seat`'s answer to the decision `id`, if that decision is pending for that seat.
    pub(crate) fn answer(
        &mut self,
        seat: PlayerId,
        id: DecisionId,
        answer: Answer,
    ) -> Result<Step, Rejection> {
        let current = self
            .pending
            .as_ref()
            .is_some_and(|p| p.id == id && p.decision.player == seat);
        if !current {
            return Err(Rejection::new(
                ErrorCode::StaleDecision,
                "that decision is not pending for you",
            ));
        }
        self.step(seat, false, |engine| engine.apply(answer))
            .map_err(|error| match error {
                EngineError::Illegal(illegal) => {
                    Rejection::new(ErrorCode::IllegalAnswer, illegal.to_string())
                }
                EngineError::NoDecision => {
                    Rejection::new(ErrorCode::StaleDecision, "no decision is pending")
                }
                _ => Rejection::engine_failed(),
            })
    }

    /// The forecast of `answer` to the decision `id`, only if that decision is pending for `seat` right now and the
    /// engine can preview the answer; nothing is logged and the engine is not changed.
    pub(crate) fn preview(
        &self,
        seat: PlayerId,
        id: DecisionId,
        answer: &Answer,
    ) -> Option<CombatForecast> {
        let current = self
            .pending
            .as_ref()
            .is_some_and(|p| p.id == id && p.decision.player == seat);
        if !current {
            return None;
        }
        self.engine.preview_forecast(seat, answer).ok()
    }

    /// Removes `seat` from the game.
    pub(crate) fn concede(&mut self, seat: PlayerId) -> Result<Step, Rejection> {
        self.step(seat, true, |engine| engine.concede(seat))
            .map_err(|error| match error {
                EngineError::NoDecision => {
                    Rejection::new(ErrorCode::BadMessage, "the game is already over")
                }
                EngineError::NotInGame(_) => {
                    Rejection::new(ErrorCode::BadMessage, "you are not in the game")
                }
                _ => Rejection::engine_failed(),
            })
    }

    fn step(
        &mut self,
        origin: PlayerId,
        keep_unchanged: bool,
        run: impl FnOnce(&mut Engine) -> Result<Vec<Event>, EngineError>,
    ) -> Result<Step, EngineError> {
        let before = self.engine.state().clone();
        let checkpoint = self.engine.checkpoint();
        let events = run(&mut self.engine)?;
        let from_seq = count(self.engine.events().len() - events.len());
        Ok(Step {
            before,
            checkpoint,
            events,
            from_seq,
            origin,
            keep_unchanged,
        })
    }
}
