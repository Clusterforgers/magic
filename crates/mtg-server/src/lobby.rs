//! The lobby: seats join and say they are ready; the last `Ready` creates the engine.

use mtg_core::ids::PlayerId;
use mtg_engine::{Engine, GameState};
use mtg_protocol::ErrorCode;

use crate::actor::{Fatal, GameActor, Phase};
use crate::pace;
use crate::running::{Rejection, Running};
use crate::store::EventStore;

impl<S: EventStore> GameActor<S> {
    /// Marks `seat` ready and starts the game once every seat is; a late `Ready` is harmless.
    pub(crate) async fn ready(&mut self, seat: PlayerId) -> Result<(), Fatal> {
        let Phase::Lobby { ready } = &mut self.phase else {
            return Ok(());
        };
        if let Some(flag) = ready.get_mut(seat.index()) {
            *flag = true;
        }
        if ready.iter().all(|r| *r) {
            return self.start(seat).await;
        }
        Ok(())
    }

    /// Creates the engine, persists its opening events and shows them from seq 0 like any other batch.
    ///
    /// If anything fails the lobby stays as it was, so a repeated `Ready` retries.
    async fn start(&mut self, trigger: PlayerId) -> Result<(), Fatal> {
        let (Ok(engine), Ok(before)) = (
            Engine::new(self.setup.clone()),
            GameState::from_setup(&self.setup),
        ) else {
            let reason = "the game could not be started";
            self.refuse(trigger, Rejection::new(ErrorCode::Internal, reason));
            return Ok(());
        };
        let opening = engine.events().to_vec();
        match self.persist(&before, &opening, 0).await {
            Ok(batches) => {
                let mut running = Running::new(engine, self.pace);
                let first = running.settle(false);
                // A game starts in the mulligans, but a scenario can open on a priority decision nobody can use.
                let idle = first
                    .as_ref()
                    .is_some_and(|issued| pace::skippable(&issued.decision, &self.auto_pass));
                self.phase = Phase::Running(Box::new(running));
                if let Some(recorder) = &self.recorder {
                    recorder.started().await;
                }
                self.announce(batches, Vec::new(), if idle { None } else { first });
                if idle {
                    return self.start_up().await;
                }
            }
            Err(error) => self.refuse(trigger, error.into_rejection()),
        }
        Ok(())
    }
}
