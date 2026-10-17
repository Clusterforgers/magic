//! Playing: answers and concessions become persisted, projected batches for every connected seat.

use mtg_core::ids::PlayerId;
use mtg_engine::{Answer, Decision, Event, GameState};
use mtg_protocol::{
    DecisionId, ErrorCode, EventBatch, ProtocolError, ServerMessage, project_events,
};
use thiserror::Error;

use crate::actor::{Fatal, GameActor, Phase, not_started};
use crate::running::{Rejection, Step};
use crate::store::{EventStore, StoreError};

/// Why a step could not be turned into something clients may see.
#[derive(Debug, Error)]
pub(crate) enum CommitError {
    /// The events could not be projected for a viewer.
    #[error("projection failed: {0}")]
    Projection(#[from] ProtocolError),
    /// The store refused the events.
    #[error("store failed: {0}")]
    Store(#[from] StoreError),
}

impl CommitError {
    /// What the client is told; the details stay on the server.
    pub(crate) fn into_rejection(self) -> Rejection {
        let reason = match self {
            CommitError::Projection(_) => "the server failed to prepare the move; nothing happened",
            CommitError::Store(_) => "the move could not be saved; nothing happened",
        };
        Rejection::new(ErrorCode::Internal, reason)
    }
}

impl<S: EventStore> GameActor<S> {
    /// Applies `seat`'s answer to the decision `id` if it is current, legal and can be saved.
    pub(crate) async fn answer(
        &mut self,
        seat: PlayerId,
        id: DecisionId,
        answer: Answer,
    ) -> Result<(), Fatal> {
        let Phase::Running(running) = &mut self.phase else {
            self.refuse(seat, not_started());
            return Ok(());
        };
        match running.answer(seat, id, answer) {
            Ok(step) => self.publish(step).await,
            Err(rejection) => {
                self.refuse(seat, rejection);
                Ok(())
            }
        }
    }

    /// Removes `seat` from the game.
    pub(crate) async fn concede(&mut self, seat: PlayerId) -> Result<(), Fatal> {
        let Phase::Running(running) = &mut self.phase else {
            self.refuse(seat, not_started());
            return Ok(());
        };
        match running.concede(seat) {
            Ok(step) => self.publish(step).await,
            Err(rejection) => {
                self.refuse(seat, rejection);
                Ok(())
            }
        }
    }

    /// Persists a step and shows it to the table, or takes it back if it cannot be saved.
    async fn publish(&mut self, step: Step) -> Result<(), Fatal> {
        let Step {
            before,
            checkpoint,
            events,
            from_seq,
            origin,
            keep_unchanged,
        } = step;
        let committed = self.persist(&before, &events, from_seq).await;
        let Phase::Running(running) = &mut self.phase else {
            return Ok(());
        };
        match committed {
            Ok(batches) => {
                let next = running.settle(keep_unchanged);
                self.announce(batches, next);
                Ok(())
            }
            Err(error) => {
                // Without the undo the game would run ahead of its record, so stop instead.
                running.engine.restore(checkpoint).map_err(|_| Fatal)?;
                self.refuse(origin, error.into_rejection());
                Ok(())
            }
        }
    }

    /// Projects `events` for the connected seats, then saves them; nothing is sent yet.
    pub(crate) async fn persist(
        &mut self,
        before: &GameState,
        events: &[Event],
        from_seq: u64,
    ) -> Result<Vec<(PlayerId, EventBatch)>, CommitError> {
        if events.is_empty() {
            return Ok(Vec::new()); // a cast in progress changes nothing worth saving or showing
        }
        let viewers = self.seats.connected();
        let batches = if viewers.is_empty() {
            Vec::new()
        } else {
            project_events(before, events, &viewers, from_seq)?
        };
        self.store.append(self.id, from_seq, events).await?;
        Ok(viewers.into_iter().zip(batches).collect())
    }

    /// Sends saved events to their seats, then the next decision to the seat it names, then the result.
    pub(crate) fn announce(
        &mut self,
        batches: Vec<(PlayerId, EventBatch)>,
        next: Option<(DecisionId, Decision)>,
    ) {
        for (seat, batch) in batches {
            self.seats.send(seat, ServerMessage::Events(batch));
        }
        if let Some((id, decision)) = next {
            let seat = decision.player;
            self.seats
                .send(seat, ServerMessage::Decision { id, decision });
        }
        let result = match &self.phase {
            Phase::Running(running) => running.engine.result(),
            Phase::Lobby { .. } => None,
        };
        if let Some(result) = result {
            self.seats.broadcast(&ServerMessage::GameOver { result });
        }
    }
}
