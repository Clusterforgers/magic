//! Playing: answers and concessions become persisted, projected batches for every connected seat.

use mtg_core::ids::PlayerId;
use mtg_engine::manual::ManualAction;
use mtg_engine::{Answer, EngineError, Event, GameState};
use mtg_protocol::{
    DecisionId, ErrorCode, EventBatch, ProtocolError, ServerMessage, project_events,
};
use thiserror::Error;

use crate::actor::{Fatal, GameActor, Phase, not_started};
use crate::running::{Halt, Issued, Rejection, Step};
use crate::stops::{self, Stops};
use crate::store::{EventStore, StoreError};
use crate::yields::Yields;

/// Why a step could not be turned into something clients may see.
#[derive(Debug, Error)]
pub(crate) enum CommitError {
    /// The events could not be projected for a viewer.
    #[error("projection failed: {0}")]
    Projection(#[from] ProtocolError),
    /// The store refused the events.
    #[error("store failed: {0}")]
    Store(#[from] StoreError),
    /// The engine failed while passing for seats that had nothing to do.
    #[error("engine failed: {0}")]
    Engine(#[from] EngineError),
}

impl CommitError {
    /// What the client is told; the details stay on the server.
    pub(crate) fn into_rejection(self) -> Rejection {
        let reason = match self {
            CommitError::Projection(_) | CommitError::Engine(_) => {
                "the server failed to prepare the move; nothing happened"
            }
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

    /// Applies `seat`'s manual action out of band, or keeps it until a replacement question is answered.
    pub(crate) async fn manual(
        &mut self,
        seat: PlayerId,
        action: ManualAction,
    ) -> Result<(), Fatal> {
        let Phase::Running(running) = &mut self.phase else {
            self.refuse(seat, not_started());
            return Ok(());
        };
        match running.manual(seat, action) {
            Ok(Some(step)) => self.publish(step).await,
            Ok(None) => Ok(()),
            Err(rejection) => {
                self.refuse(seat, rejection);
                Ok(())
            }
        }
    }

    /// Applies the manual actions that waited for a replacement question, once it is answered.
    pub(crate) async fn flush_deferred(&mut self) -> Result<(), Fatal> {
        loop {
            let Phase::Running(running) = &mut self.phase else {
                return Ok(());
            };
            let Some((seat, action)) = running.take_deferred() else {
                return Ok(());
            };
            self.manual(seat, action).await?;
        }
    }

    /// Persists a step and shows it to the table, or takes it back if it cannot be saved.
    ///
    /// First the seats the windows pass for at once ([`crate::windows`]: a seat that is not in Full Control and meets no
    /// window of its own, or that yields) are passed for, so the step, the passes that follow it and the save are
    /// one unit: either the whole stretch is kept or none of it. A seat that must wait at its window (`Window::dwells`)
    /// is not passed for here: the run stops at it and the pass comes as a step of its own when its dwell is over
    /// ([`GameActor::expire`]). The yields and the stops (with the modes) start from copies that replace the seats' own
    /// only once saved.
    pub(crate) async fn publish(&mut self, step: Step) -> Result<(), Fatal> {
        let yields = self.yields.clone();
        let stops = self.stops.clone();
        self.publish_with(step, yields, stops).await
    }

    /// [`publish`](GameActor::publish) with the yields and the stops the step starts from, which are kept only if
    /// it is saved.
    pub(crate) async fn publish_with(
        &mut self,
        step: Step,
        mut yields: Yields,
        mut stops: Stops,
    ) -> Result<(), Fatal> {
        let Step {
            before,
            checkpoint,
            mut events,
            from_seq,
            origin,
            keep_unchanged,
            passed,
            banks,
        } = step;
        let Phase::Running(running) = &mut self.phase else {
            return Ok(());
        };
        // The step's own events come first, so the passes that follow already meet the stops and the mode it left.
        stops.left(stops::place(&before), &events);
        stops.resolved(&stops::by_hand_on_stack(&before), &events);
        if passed {
            stops.passed(origin);
        }
        let skipped = running.skip_idle(&mut events, &mut stops, &mut yields);
        let halt = skipped.as_ref().ok().copied().unwrap_or(Halt::Ask);
        let committed = match skipped {
            Ok(_) => self.persist(&before, &events, from_seq).await,
            Err(error) => Err(CommitError::from(error)),
        };
        let Phase::Running(running) = &mut self.phase else {
            return Ok(());
        };
        match committed {
            Ok(mut batches) => {
                for (_, batch) in &mut batches {
                    running.stamp(&mut batch.table);
                }
                yields.drop_finished(running.engine.state());
                stops.drop_finished(running.engine.state());
                let mut notices: Vec<(PlayerId, ServerMessage)> = self
                    .yields
                    .replace(yields)
                    .into_iter()
                    .map(|(seat, until)| (seat, ServerMessage::Yielding { until }))
                    .collect();
                notices.extend(self.stops.replace(stops));
                let next = running.settle(keep_unchanged, halt);
                let result = running.engine.result();
                // Before anyone hears `GameOver`, so a client that saw it can trust the record.
                if let (Some(recorder), Some(result)) = (&self.recorder, result) {
                    recorder.finished(result).await;
                }
                self.announce(batches, notices, next);
                Ok(())
            }
            Err(error) => {
                // Without the undo the game would run ahead of its record, so stop instead.
                running.engine.restore(checkpoint).map_err(|_| Fatal)?;
                running.restore_banks(banks);
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

    /// Sends saved events to their seats, then each notice of a change (yields, then stops and Full Control), then
    /// each next decision to the seat it names, then the result.
    ///
    /// A yield that ended is told before the decision that ended it, so the prompt never appears under a
    /// "passing" notice; the stops are told in the same place.
    pub(crate) fn announce(
        &mut self,
        batches: Vec<(PlayerId, EventBatch)>,
        notices: Vec<(PlayerId, ServerMessage)>,
        next: Vec<Issued>,
    ) {
        for (seat, batch) in batches {
            self.seats.send(seat, ServerMessage::Events(batch));
        }
        for (seat, notice) in notices {
            self.seats.send(seat, notice);
        }
        for Issued {
            id,
            decision,
            deadline_ms,
            show_after_ms,
        } in next
        {
            let seat = decision.player;
            self.seats.send(
                seat,
                ServerMessage::Decision {
                    id,
                    decision,
                    deadline_ms,
                    show_after_ms,
                },
            );
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
