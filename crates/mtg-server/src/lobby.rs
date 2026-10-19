//! The lobby: seats join and say they are ready; the last `Ready` creates the engine.
//!
//! Every connected seat is kept told who is at the table ([`ServerMessage::Lobby`]); readiness lives
//! here only and is never stored, so a lobby resumed after a restart starts with nobody ready.

use mtg_core::ids::PlayerId;
use mtg_engine::{Engine, GameState};
use mtg_protocol::{ErrorCode, LobbySeat, ServerMessage};

use crate::actor::{Fatal, GameActor, Phase};
use crate::running::{Halt, Rejection, Running};
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

    /// Puts `name` at `seat` of the lobby, or empties the seat for `None`, and tells the table.
    ///
    /// Emptying a seat closes its connection and takes its `Ready` back, so nothing of the person
    /// who left stays at the table. Returns false, with nothing changed, once the game runs or for
    /// a seat the table does not have.
    pub(crate) fn seat_at(&mut self, seat: PlayerId, name: Option<String>) -> bool {
        let Phase::Lobby { ready, names } = &mut self.phase else {
            return false;
        };
        let (Some(listed), Some(flag)) = (names.get_mut(seat.index()), ready.get_mut(seat.index()))
        else {
            return false;
        };
        if let Some(name) = name {
            *listed = name;
        } else {
            listed.clear();
            *flag = false;
            self.seats.detach(seat);
            // Whoever takes the seat next starts from the server's defaults, not from these stops.
            self.stops.reset(seat, self.pace.mode());
            if let Some(recorder) = &mut self.recorder {
                recorder.forget(seat);
            }
        }
        self.push_roster();
        true
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
                let first = running.settle(false, Halt::Ask);
                // A game starts in the mulligans, but a scenario can open on a priority decision nobody is asked.
                let idle = first.as_ref().is_some_and(|issued| {
                    running.passes_pending(issued.decision.player, &self.stops, &self.yields)
                });
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

#[cfg(test)]
mod tests {
    //! Goal: the registry can name and empty a seat of the lobby: the table is told each time, emptying closes the seat's connection and takes its Ready back, and once the game runs or for a seat the table does not have nothing changes and nothing is sent.
    use mtg_core::ids::PlayerId;
    use mtg_protocol::{ClientMessage, LobbySeat, ServerMessage};
    use mtg_testkit::fuzz_setup;
    use tokio::sync::mpsc::Receiver;

    use crate::actor::{GameConfig, spawn_game_with};
    use crate::game_id::GameId;
    use crate::handle::GameHandle;
    use crate::pace::Pace;
    use crate::store::MemoryStore;

    fn lobby(seats: usize) -> GameHandle {
        let config = GameConfig {
            seat_queue: 64,
            command_queue: 64,
            pace: Pace::MANUAL,
        };
        let setup = fuzz_setup(seats, 3);
        spawn_game_with(GameId::random(), setup, MemoryStore::new(), config).unwrap()
    }

    fn drain(inbox: &mut Receiver<ServerMessage>) -> Vec<ServerMessage> {
        std::iter::from_fn(|| inbox.try_recv().ok()).collect()
    }

    /// The seats of the one roster in `messages`.
    fn only_roster(messages: &[ServerMessage]) -> &[LobbySeat] {
        match messages {
            [ServerMessage::Lobby { seats, .. }] => seats,
            other => panic!("expected one roster, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn naming_and_emptying_a_seat_tells_the_table_and_closes_the_seat() {
        let handle = lobby(3);
        let (mut first, mut second) = (
            handle.join(PlayerId(0)).await.unwrap(),
            handle.join(PlayerId(1)).await.unwrap(),
        );
        handle
            .send(PlayerId(1), ClientMessage::Ready)
            .await
            .unwrap();
        // The actor handles commands in order, so the roster of the `Ready` is queued by now.
        handle.status().await.unwrap();
        drain(&mut first);
        drain(&mut second);

        assert_eq!(handle.occupy(PlayerId(2), "Cy".to_owned()).await, Ok(true));
        for inbox in [&mut first, &mut second] {
            let messages = drain(inbox);
            let roster = only_roster(&messages);
            assert_eq!(roster[2].name, "Cy");
            assert!(!roster[2].connected && !roster[2].ready);
        }

        // Seat 1 is ready and connected; emptying it takes both away.
        assert_eq!(handle.vacate(PlayerId(1)).await, Ok(true));
        let messages = drain(&mut first);
        let roster = only_roster(&messages);
        assert_eq!(roster[1].name, "");
        assert!(!roster[1].connected && !roster[1].ready);
        assert_eq!(roster[2].name, "Cy", "the other seats keep their names");
        assert_eq!(drain(&mut second), []);
        assert!(second.recv().await.is_none(), "the queue of the seat ends");
        assert_eq!(handle.status().await.unwrap().ready, []);
    }

    #[tokio::test]
    async fn nothing_changes_for_a_seat_the_table_does_not_have() {
        let handle = lobby(2);
        let mut inbox = handle.join(PlayerId(0)).await.unwrap();
        drain(&mut inbox);
        assert_eq!(handle.occupy(PlayerId(5), "Cy".to_owned()).await, Ok(false));
        assert_eq!(handle.vacate(PlayerId(5)).await, Ok(false));
        handle.status().await.unwrap();
        assert_eq!(drain(&mut inbox), []);
    }

    #[tokio::test]
    async fn once_the_game_runs_a_seat_can_no_longer_be_named_or_emptied() {
        let handle = lobby(2);
        let mut inbox = handle.join(PlayerId(0)).await.unwrap();
        for seat in [PlayerId(0), PlayerId(1)] {
            handle.send(seat, ClientMessage::Ready).await.unwrap();
        }
        assert!(handle.status().await.unwrap().started);
        drain(&mut inbox);

        assert_eq!(handle.occupy(PlayerId(1), "Cy".to_owned()).await, Ok(false));
        assert_eq!(handle.vacate(PlayerId(1)).await, Ok(false));
        handle.status().await.unwrap();
        assert_eq!(drain(&mut inbox), [], "no roster once the game runs");
        let connected = handle.status().await.unwrap().connected;
        assert!(connected.contains(&PlayerId(0)));
    }
}
