//! Manual actions out of band: a seat in the game does something by hand at any time, whoever the pending decision
//! belongs to, and a decision the change alters is issued again with the clock it had.

use std::mem::discriminant;

use mtg_core::ids::PlayerId;
use mtg_engine::manual::ManualAction;
use mtg_engine::{Decision, EngineError};
use mtg_protocol::{DecisionId, ErrorCode};
use tokio::time::Instant;

use super::{Issued, Pending, Rejection, Running, Step, millis};

impl Running {
    /// `seat`'s manual action as a step, or `None` while a replacement question holds a batch: the action then waits
    /// in order and [`take_deferred`](Running::take_deferred) hands it back once the question is answered.
    pub(crate) fn manual(
        &mut self,
        seat: PlayerId,
        action: ManualAction,
    ) -> Result<Option<Step>, Rejection> {
        let state = self.engine.state();
        if self.engine.result().is_some() {
            return Err(Rejection::new(ErrorCode::BadMessage, "the game is over"));
        }
        if !state.is_alive(seat) {
            return Err(Rejection::new(
                ErrorCode::BadMessage,
                "you are not in the game",
            ));
        }
        if self.engine.replacing() {
            self.deferred.push_back((seat, action));
            return Ok(None);
        }
        let step = self
            .step(seat, true, |engine| engine.manual(seat, &action))
            .map_err(|error| match error {
                EngineError::Illegal(illegal) => {
                    Rejection::new(ErrorCode::IllegalAnswer, illegal.to_string())
                }
                EngineError::NoDecision => {
                    Rejection::new(ErrorCode::BadMessage, "the game is over")
                }
                EngineError::NotInGame(_) => {
                    Rejection::new(ErrorCode::BadMessage, "you are not in the game")
                }
                _ => Rejection::engine_failed(),
            })?;
        self.clock_kept = true;
        Ok(Some(step))
    }

    /// The oldest manual action that waited for a replacement question, once none holds a batch.
    pub(crate) fn take_deferred(&mut self) -> Option<(PlayerId, ManualAction)> {
        if self.engine.replacing() {
            return None;
        }
        self.deferred.pop_front()
    }

    /// After a manual step, the clock (`due`, `shown_at`) of the pending decision when the new one is the same seat's
    /// and of the same kind, so a change by hand never starts or resets a clock. Clears the mark either way.
    pub(super) fn carried_clock(
        &mut self,
        decision: &Decision,
        dwell: bool,
    ) -> Option<(Option<Instant>, Option<Instant>)> {
        if !std::mem::take(&mut self.clock_kept) || dwell {
            return None;
        }
        self.pending
            .as_ref()
            .filter(|p| {
                !p.hidden
                    && p.decision.player == decision.player
                    && discriminant(&p.decision.kind) == discriminant(&decision.kind)
            })
            .map(|p| (p.due, p.shown_at))
    }

    /// Issues `decision` under the new `id` with the clock it carried over.
    pub(super) fn reissue(
        &mut self,
        id: DecisionId,
        decision: Decision,
        due: Option<Instant>,
        shown_at: Option<Instant>,
    ) -> Issued {
        let now = Instant::now();
        let deadline_ms = due.map(|due| millis(due.saturating_duration_since(now)));
        let show_after_ms = shown_at
            .map(|at| millis(at.saturating_duration_since(now)))
            .filter(|ms| *ms > 0);
        self.pending = Some(Pending {
            id,
            decision: decision.clone(),
            due,
            shown_at,
            hidden: false,
        });
        Issued {
            id,
            decision,
            deadline_ms,
            show_after_ms,
        }
    }
}
