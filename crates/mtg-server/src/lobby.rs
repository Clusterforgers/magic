//! The lobby: seats join and say they are ready; the last `Ready` creates the engine.
//!
//! Every connected seat is kept told who is at the table ([`ServerMessage::Lobby`]); readiness lives
//! here only and is never stored, so a lobby resumed after a restart starts with nobody ready.

use mtg_core::ids::PlayerId;
use mtg_engine::{Engine, GameState};
use mtg_protocol::{ErrorCode, LobbySeat, ServerMessage};

use crate::actor::{Fatal, GameActor, Phase};
use crate::pace;
use crate::running::{Rejection, Running};
use crate::store::EventStore;

impl<S: EventStore> GameActor<S> {
    /// Whether the game is still collecting seats and readiness.
    pub(crate) fn in_lobby(&self) -> bool {
        matches!(self.phase, Phase::Lobby { .. })
    }

    /// Marks `seat` ready and starts the game once every seat is; a late `Ready` is harmless.
    pub(crate) async fn ready(&mut self, seat: PlayerId) -> Result<(), Fatal> {
        let Phase::Lobby { ready, .. } = &mut self.phase else {
            return Ok(());
        };
        let Some(flag) = ready.get_mut(seat.index()) else {
            return Ok(());
        };
        let changed = !std::mem::replace(flag, true);
        if ready.iter().all(|r| *r) {
            self.start(seat).await?;
            // The opening events are the news once the game began; a start that failed leaves the lobby as it is.
            if !self.in_lobby() {
                return Ok(());
            }
        }
        if changed {
            self.push_roster();
        }
        Ok(())
    }

    /// Takes `seat`'s `Ready` back while the lobby is open; harmless once the game began.
    pub(crate) fn not_ready(&mut self, seat: PlayerId) {
        let Phase::Lobby { ready, .. } = &mut self.phase else {
            return;
        };
        if let Some(flag) = ready.get_mut(seat.index())
            && std::mem::replace(flag, false)
        {
            self.push_roster();
        }
    }

    /// Sends the roster to `seat` alone, for a connection that took the seat over from another.
    pub(crate) fn send_roster(&mut self, seat: PlayerId) {
        let connected = self.seats.connected();
        if let Some(message) = self.roster(&connected) {
            self.seats.send(seat, message);
        }
    }

    /// Tells every connected seat who is at the table and who is ready; nothing once the game runs.
    pub(crate) fn push_roster(&mut self) {
        // A client that cannot take the message is dropped by the send. Everybody else must hear of the gap it
        // leaves, and each further round means one more client went, so the loop ends.
        for _ in 0..=self.seats.len() {
            let connected = self.seats.connected();
            // Whoever the sweep found gone is not in the roster below, so there is nothing left to tell about them.
            self.seats.take_gone();
            let Some(message) = self.roster(&connected) else {
                return;
            };
            self.seats.broadcast(&message);
            if !self.seats.take_gone() {
                return;
            }
        }
    }

    /// Tells a lobby of the clients that went away without leaving: hung up, or fell too far behind.
    ///
    /// Nobody says so, so it is looked for after every command (any of them may be the one that finds a closed queue
    /// first) and whenever a queue closes while the actor is idle.
    pub(crate) fn announce_departures(&mut self) {
        if !self.in_lobby() {
            self.seats.take_gone();
            return;
        }
        self.seats.connected();
        if self.seats.take_gone() {
            self.push_roster();
        }
    }

    /// The lobby as the seats see it, `None` once the game runs; `connected` are the seats with a live client.
    fn roster(&self, connected: &[PlayerId]) -> Option<ServerMessage> {
        let Phase::Lobby { ready, names } = &self.phase else {
            return None;
        };
        let seats: Vec<LobbySeat> = names
            .iter()
            .zip(ready)
            .enumerate()
            .map(|(index, (name, ready))| {
                let seat = PlayerId::from_index(index);
                LobbySeat {
                    seat,
                    name: name.clone(),
                    connected: connected.contains(&seat),
                    ready: *ready,
                }
            })
            .collect();
        let needed = seats.iter().filter(|seat| !seat.ready).count();
        Some(ServerMessage::Lobby {
            seats,
            needed: u32::try_from(needed).unwrap_or(u32::MAX),
        })
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
