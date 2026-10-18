//! A started game as a plain state machine: the engine plus the decision ids that tell stale answers from current ones.

use std::time::Duration;

use mtg_core::ids::PlayerId;
use mtg_engine::{Answer, Checkpoint, Decision, Engine, EngineError, Event, GameState};
use mtg_protocol::{DecisionId, ErrorCode, ServerMessage, StateView};
use tokio::time::Instant;

use crate::pace::{self, Pace};
use crate::yields::Yields;

/// Most priority passes made for idle seats in one go; past it the decision is sent as it is.
///
/// A real game needs a handful. The cap keeps one batch small and the actor responsive if a table
/// ever offers nothing to do for dozens of turns.
const MAX_IDLE_PASSES: usize = 500;

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

/// The decision awaiting an answer, the id the client must echo and when its clock runs out.
#[derive(Debug)]
struct Pending {
    id: DecisionId,
    decision: Decision,
    /// When the server answers for the seat; `None` for a decision without a clock.
    due: Option<Instant>,
}

/// A decision just issued: what its seat is sent.
#[derive(Debug)]
pub(crate) struct Issued {
    pub(crate) id: DecisionId,
    pub(crate) decision: Decision,
    /// Milliseconds on its clock, if it has one.
    pub(crate) deadline_ms: Option<u64>,
}

/// A started game.
#[derive(Debug)]
pub(crate) struct Running {
    pub(crate) engine: Engine,
    pending: Option<Pending>,
    last_id: u64,
    pace: Pace,
}

pub(crate) fn count(len: usize) -> u64 {
    u64::try_from(len).unwrap_or(u64::MAX)
}

/// Whole milliseconds, saturating.
fn millis(span: Duration) -> u64 {
    u64::try_from(span.as_millis()).unwrap_or(u64::MAX)
}

impl Running {
    /// Wraps `engine`; call [`settle`](Running::settle) to issue the first decision id.
    pub(crate) fn new(engine: Engine, pace: Pace) -> Running {
        Running {
            engine,
            pending: None,
            last_id: 0,
            pace,
        }
    }

    /// The seat that must answer, if any.
    pub(crate) fn waiting_on(&self) -> Option<PlayerId> {
        self.pending.as_ref().map(|p| p.decision.player)
    }

    /// When the pending decision's clock runs out, if it has one.
    pub(crate) fn due(&self) -> Option<Instant> {
        self.pending.as_ref().and_then(|p| p.due)
    }

    /// Moves the clock of the pending decision to `due` (nothing happens for a decision without one).
    pub(crate) fn postpone(&mut self, due: Instant) {
        if let Some(pending) = self.pending.as_mut().filter(|p| p.due.is_some()) {
            pending.due = Some(due);
        }
    }

    /// Drops the clock of the pending decision, so a decision the server cannot answer for is not retried forever.
    pub(crate) fn stop_clock(&mut self) {
        if let Some(pending) = self.pending.as_mut() {
            pending.due = None;
        }
    }

    /// Whether the pending decision is `seat`'s, cannot be used and `seat` has auto-pass on.
    pub(crate) fn idle_for(&self, seat: PlayerId, auto_pass: &[bool]) -> bool {
        self.pending
            .as_ref()
            .is_some_and(|p| p.decision.player == seat && pace::skippable(&p.decision, auto_pass))
    }

    /// Aligns the pending decision with the engine's and returns what a client must be sent, if anything.
    ///
    /// Every new decision gets a fresh id and, when [`Pace`] gives it one, a clock that starts now. With
    /// `keep_unchanged`, a decision that did not change keeps its id and clock and is not sent again, so a
    /// concession elsewhere does not invalidate a pending answer.
    pub(crate) fn settle(&mut self, keep_unchanged: bool) -> Option<Issued> {
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
        let clock = self
            .pace
            .clock_for(&decision, self.engine.state().turn().active());
        let due = clock.and_then(|span| Instant::now().checked_add(span));
        self.pending = Some(Pending {
            id,
            decision: decision.clone(),
            due,
        });
        Some(Issued {
            id,
            decision,
            deadline_ms: clock.map(millis),
        })
    }

    /// What a client needs to catch up: a snapshot, its pending decision and the result if the game is over.
    ///
    /// The decision carries the time its clock has left, not the time it started with.
    pub(crate) fn sync_messages(&self, seat: PlayerId) -> Vec<ServerMessage> {
        let mut messages = vec![ServerMessage::Snapshot {
            seq: count(self.engine.events().len()),
            state: Box::new(StateView::for_player(self.engine.state(), seat)),
        }];
        if let Some(pending) = self.pending.as_ref().filter(|p| p.decision.player == seat) {
            let now = Instant::now();
            messages.push(ServerMessage::Decision {
                id: pending.id,
                decision: pending.decision.clone(),
                deadline_ms: pending
                    .due
                    .map(|due| millis(due.saturating_duration_since(now))),
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

    /// Answers the pending decision for its seat with the safe default: a pass, or no blocks.
    ///
    /// It goes through [`answer`](Running::answer), so it is checked and logged like the seat's own move.
    pub(crate) fn fall_back(&mut self) -> Result<Step, Rejection> {
        let pending = self.pending.as_ref();
        let Some((id, seat, answer)) = pending.and_then(|p| {
            pace::fallback(&p.decision).map(|answer| (p.id, p.decision.player, answer))
        }) else {
            return Err(Rejection::new(
                ErrorCode::StaleDecision,
                "no decision is pending that can be answered for its seat",
            ));
        };
        self.answer(seat, id, answer)
    }

    /// Passes for every seat that has nothing to do and has auto-pass on, and for every seat that yields,
    /// until some decision needs a player.
    ///
    /// The passes go through the engine like any answer; their events are added to `events`, so the
    /// whole stretch is saved and shown as one batch. A seat that is asked anything else, or that can
    /// act (unless it yields), stops the run, and so does the end of the game. A yield that reaches its end
    /// on the way is dropped from `yields`, which the caller keeps only if the stretch is saved.
    pub(crate) fn skip_idle(
        &mut self,
        events: &mut Vec<Event>,
        auto_pass: &[bool],
        yields: &mut Yields,
    ) -> Result<(), EngineError> {
        self.skip_up_to(MAX_IDLE_PASSES, events, auto_pass, yields)
    }

    /// [`skip_idle`](Running::skip_idle) with a limit on the passes, which a test can make small.
    fn skip_up_to(
        &mut self,
        limit: usize,
        events: &mut Vec<Event>,
        auto_pass: &[bool],
        yields: &mut Yields,
    ) -> Result<(), EngineError> {
        for _ in 0..limit {
            let Some(decision) = self.engine.decision() else {
                return Ok(());
            };
            if !pace::passes_for(decision, auto_pass, yields, self.engine.state()) {
                return Ok(());
            }
            events.extend(self.engine.apply(pace::pass())?);
        }
        // Cut short: the next priority decision is sent as it is, so a yield it belongs to ends here instead of
        // leaving a seat that "passes for it" with a prompt to answer.
        if let Some(decision) = self.engine.decision().filter(|d| pace::is_priority(d)) {
            yields.cancel(decision.player);
        }
        Ok(())
    }

    /// Whether the pending decision is a priority decision of `seat`.
    pub(crate) fn asks_priority_of(&self, seat: PlayerId) -> bool {
        self.pending
            .as_ref()
            .is_some_and(|p| p.decision.player == seat && pace::is_priority(&p.decision))
    }

    /// A step that changes nothing, so that the pending decision of `seat` meets a new yield and, if the
    /// yield does not pass for it, is issued again with a new id.
    pub(crate) fn restart_decision(&mut self, seat: PlayerId) -> Result<Step, Rejection> {
        self.step(seat, false, |_| Ok(Vec::new()))
            .map_err(|_| Rejection::engine_failed())
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

#[cfg(test)]
mod tests {
    //! Goal: a run of passes stops at its limit with the next priority decision still pending, and a yield that decision belongs to ends there, so the seat is never handed a prompt while the server still "passes for it"; every other seat is left alone.
    use super::*;
    use mtg_protocol::YieldUntil;
    use mtg_testkit::Scenario;
    use rstest::rstest;

    const SEATS: [PlayerId; 2] = [PlayerId(0), PlayerId(1)];

    fn running() -> Running {
        let setup = Scenario::new(2).setup().unwrap();
        let mut running = Running::new(Engine::new(setup).unwrap(), Pace::MANUAL);
        running.settle(false);
        running
    }

    fn yielding(running: &Running, seats: &[PlayerId]) -> Yields {
        let mut yields = Yields::new(2);
        for seat in seats {
            yields.start(*seat, YieldUntil::EndOfTurn, running.engine.state());
        }
        yields
    }

    #[rstest]
    #[case::the_cut_lands_on_a_seat_that_yields(2, &[SEATS[0], SEATS[1]], &[SEATS[1]])]
    #[case::the_cut_lands_on_a_seat_that_does_not(1, &[SEATS[0]], &[SEATS[0]])]
    fn the_cut_ends_the_yield_of_the_seat_whose_decision_is_next(
        #[case] limit: usize,
        #[case] before: &[PlayerId],
        #[case] after: &[PlayerId],
    ) {
        let mut running = running();
        let mut yields = yielding(&running, before);
        let mut events = Vec::new();
        running
            .skip_up_to(limit, &mut events, &[false, false], &mut yields)
            .unwrap();
        let passes = events
            .iter()
            .filter(|event| matches!(event, Event::PriorityPassed { .. }))
            .count();
        assert_eq!(passes, limit, "one pass for each of the limit");
        let held: Vec<PlayerId> = SEATS
            .into_iter()
            .filter(|seat| yields.get(*seat).is_some())
            .collect();
        assert_eq!(held, after);
        assert!(
            running.engine.decision().is_some(),
            "the decision is still there to be sent"
        );
    }

    #[test]
    fn without_a_limit_in_the_way_the_run_goes_on_until_a_seat_must_decide() {
        let mut running = running();
        let mut yields = yielding(&running, &[SEATS[0]]);
        let mut events = Vec::new();
        running
            .skip_idle(&mut events, &[false, false], &mut yields)
            .unwrap();
        assert_eq!(events.len(), 1, "seat 1 does not yield and is asked");
        assert_eq!(running.engine.decision().map(|d| d.player), Some(SEATS[1]));
        assert_eq!(yields.get(SEATS[0]), Some(YieldUntil::EndOfTurn));
    }
}
