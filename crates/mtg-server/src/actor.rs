//! The game actor: one task per game owns the engine, the store handle and every seat's queue.
//!
//! Clients never touch the engine. They send [`ClientMessage`]s through a [`GameHandle`] and read
//! [`ServerMessage`]s from a bounded queue, so a slow or vanished client can never stall a game.

use mtg_core::ids::PlayerId;
use mtg_engine::error::SetupError;
use mtg_engine::{Engine, EngineError, GameSetup};
use mtg_protocol::{ClientMessage, ErrorCode, PROTOCOL_VERSION, ServerMessage};
use thiserror::Error;
use tokio::sync::{mpsc, oneshot};

use crate::game_id::GameId;
use crate::handle::{Command, GameHandle, GameStatus, JoinError};
use crate::running::{Rejection, Running, count};
use crate::seats::Seats;
use crate::store::{EventStore, StoreError};

/// Longest chat line, in characters.
pub const MAX_CHAT_CHARS: usize = 500;

/// Tunables of one game actor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GameConfig {
    /// Messages a seat's queue holds before that client is dropped (never less than [`crate::MIN_QUEUE`]).
    pub seat_queue: usize,
    /// Client messages that may wait for the actor before senders have to wait.
    pub command_queue: usize,
}

impl Default for GameConfig {
    fn default() -> Self {
        GameConfig {
            seat_queue: 256,
            command_queue: 64,
        }
    }
}

/// Why a game could not be started or resumed.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum StartError {
    /// The setup is not playable.
    #[error("invalid game setup: {0}")]
    Setup(#[from] SetupError),
    /// The stored log does not replay.
    #[error("the stored game cannot be rebuilt: {0}")]
    Engine(#[from] EngineError),
    /// The store failed.
    #[error("store failed: {0}")]
    Store(#[from] StoreError),
}

/// Starts a game in its lobby; it begins once every seat has joined and sent `Ready`.
pub fn spawn_game<S: EventStore>(
    id: GameId,
    setup: GameSetup,
    store: S,
) -> Result<GameHandle, StartError> {
    spawn_game_with(id, setup, store, GameConfig::default())
}

/// Like [`spawn_game`] with explicit tunables.
pub fn spawn_game_with<S: EventStore>(
    id: GameId,
    setup: GameSetup,
    store: S,
    config: GameConfig,
) -> Result<GameHandle, StartError> {
    setup.validate()?;
    let phase = Phase::Lobby {
        ready: vec![false; setup.players.len()],
    };
    Ok(launch(id, setup, store, config, phase))
}

/// Rebuilds a game from its stored log and runs it, waiting for its players to reconnect.
///
/// A game with nothing stored never started, so it returns to its lobby.
pub async fn resume_game<S: EventStore>(
    store: S,
    id: GameId,
    setup: GameSetup,
) -> Result<GameHandle, StartError> {
    resume_game_with(store, id, setup, GameConfig::default()).await
}

/// Like [`resume_game`] with explicit tunables.
pub async fn resume_game_with<S: EventStore>(
    store: S,
    id: GameId,
    setup: GameSetup,
    config: GameConfig,
) -> Result<GameHandle, StartError> {
    let stored = store.load(id).await?;
    if stored.is_empty() {
        return spawn_game_with(id, setup, store, config);
    }
    let stored_len = stored.len();
    let engine = Engine::resume(setup.clone(), stored)?;
    // Rebuilding can run past the last stored decision; keep the store in step with the engine.
    let fresh = &engine.events()[stored_len..];
    if !fresh.is_empty() {
        store.append(id, count(stored_len), fresh).await?;
    }
    let mut running = Running::new(engine);
    running.settle(false);
    Ok(launch(
        id,
        setup,
        store,
        config,
        Phase::Running(Box::new(running)),
    ))
}

fn launch<S: EventStore>(
    id: GameId,
    setup: GameSetup,
    store: S,
    config: GameConfig,
    phase: Phase,
) -> GameHandle {
    let (commands, inbox) = mpsc::channel(config.command_queue.max(1));
    let actor = GameActor {
        id,
        store,
        seats: Seats::new(setup.players.len(), config.seat_queue),
        setup,
        phase,
    };
    tokio::spawn(actor.run(inbox));
    GameHandle::new(id, commands)
}

/// Where the game is in its life.
#[derive(Debug)]
pub(crate) enum Phase {
    /// Collecting joins and readiness; the engine does not exist yet.
    Lobby { ready: Vec<bool> },
    /// The game is on (or over, once it has a result).
    Running(Box<Running>),
}

/// The actor can no longer trust its own state and must stop.
#[derive(Debug)]
pub(crate) struct Fatal;

pub(crate) struct GameActor<S> {
    pub(crate) id: GameId,
    pub(crate) store: S,
    pub(crate) setup: GameSetup,
    pub(crate) seats: Seats,
    pub(crate) phase: Phase,
}

impl<S: EventStore> GameActor<S> {
    /// Handles commands one at a time until every handle is gone.
    async fn run(mut self, mut inbox: mpsc::Receiver<Command>) {
        while let Some(command) = inbox.recv().await {
            if self.handle(command).await.is_err() {
                return;
            }
        }
    }

    async fn handle(&mut self, command: Command) -> Result<(), Fatal> {
        match command {
            Command::Join { seat, reply } => self.join(seat, reply),
            Command::Message { seat, message } => return self.message(seat, message).await,
            Command::Leave { seat } => self.seats.detach(seat),
            Command::Status { reply } => {
                // A caller that stopped waiting needs no answer.
                let _ = reply.send(self.status());
            }
        }
        Ok(())
    }

    fn join(
        &mut self,
        seat: PlayerId,
        reply: oneshot::Sender<Result<mpsc::Receiver<ServerMessage>, JoinError>>,
    ) {
        let Some(inbox) = self.seats.attach(seat) else {
            // A caller that stopped waiting needs no answer.
            let _ = reply.send(Err(JoinError::NoSuchSeat(seat)));
            return;
        };
        let welcome = ServerMessage::Welcome {
            protocol_version: PROTOCOL_VERSION,
            game: self.id.to_string(),
            player: seat,
            seats: u32::try_from(self.seats.len()).unwrap_or(u32::MAX),
        };
        self.seats.send(seat, welcome);
        self.sync(seat);
        let _ = reply.send(Ok(inbox));
    }

    /// Queues a snapshot, the seat's pending decision and the result for a game that is running.
    fn sync(&mut self, seat: PlayerId) {
        if let Phase::Running(running) = &self.phase {
            for message in running.sync_messages(seat) {
                self.seats.send(seat, message);
            }
        }
    }

    async fn message(&mut self, seat: PlayerId, message: ClientMessage) -> Result<(), Fatal> {
        if seat.index() >= self.seats.len() {
            return Ok(()); // no such seat, so nowhere to send a reply
        }
        match message {
            ClientMessage::Join { .. } => self.refuse(
                seat,
                Rejection::new(ErrorCode::BadMessage, "join through the game handle"),
            ),
            ClientMessage::Chat { text } => self.chat(seat, text),
            ClientMessage::Ready => return self.ready(seat).await,
            ClientMessage::Answer { decision, answer } => {
                return self.answer(seat, decision, answer).await;
            }
            ClientMessage::Concede => return self.concede(seat).await,
            ClientMessage::Resync => match self.phase {
                Phase::Running(_) => self.sync(seat),
                Phase::Lobby { .. } => self.refuse(seat, not_started()),
            },
        }
        Ok(())
    }

    fn chat(&mut self, seat: PlayerId, text: String) {
        if text.chars().count() > MAX_CHAT_CHARS {
            let reason = format!("chat lines are at most {MAX_CHAT_CHARS} characters");
            self.refuse(seat, Rejection::new(ErrorCode::BadMessage, reason));
        } else {
            self.seats
                .broadcast(&ServerMessage::Chat { from: seat, text });
        }
    }

    pub(crate) fn refuse(&mut self, seat: PlayerId, rejection: Rejection) {
        self.seats.send(seat, rejection.into_message());
    }

    fn status(&mut self) -> GameStatus {
        let connected = self.seats.connected();
        match &self.phase {
            Phase::Lobby { .. } => GameStatus {
                started: false,
                events: 0,
                state_hash: None,
                waiting_on: None,
                result: None,
                connected,
            },
            Phase::Running(running) => GameStatus {
                started: true,
                events: count(running.engine.events().len()),
                state_hash: Some(running.engine.state_hash()),
                waiting_on: running.waiting_on(),
                result: running.engine.result(),
                connected,
            },
        }
    }
}

/// The refusal for anything that needs a running game.
pub(crate) fn not_started() -> Rejection {
    Rejection::new(ErrorCode::BadMessage, "the game has not started")
}
