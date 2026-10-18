//! The game actor: one task per game owns the engine, the store handle and every seat's queue.
//!
//! Clients never touch the engine. They send [`ClientMessage`]s through a [`GameHandle`] and read
//! [`ServerMessage`]s from a bounded queue, so a slow or vanished client can never stall a game.

use std::time::Instant;

use mtg_core::ids::PlayerId;
use mtg_engine::error::SetupError;
use mtg_engine::{Engine, EngineError, GameSetup};
use mtg_protocol::{ClientMessage, ErrorCode, PROTOCOL_VERSION, ServerMessage};
use thiserror::Error;
use tokio::sync::{mpsc, oneshot};

use crate::game_id::GameId;
use crate::handle::{Command, GameHandle, GameStatus, JoinError};
use crate::pace::Pace;
use crate::preview::PreviewLimits;
use crate::recorder::Recorder;
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
    /// Auto-pass and the response clocks.
    pub pace: Pace,
}

impl Default for GameConfig {
    fn default() -> Self {
        GameConfig {
            seat_queue: 256,
            command_queue: 64,
            pace: Pace::default(),
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
    spawn_recorded(id, setup, store, config, None)
}

/// Starts a lobby whose milestones (joins, start, end) are written to `recorder`.
pub(crate) fn spawn_recorded<S: EventStore>(
    id: GameId,
    setup: GameSetup,
    store: S,
    config: GameConfig,
    recorder: Option<Recorder>,
) -> Result<GameHandle, StartError> {
    setup.validate()?;
    let phase = Phase::Lobby {
        ready: vec![false; setup.players.len()],
    };
    Ok(launch(id, setup, store, config, phase, recorder))
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
    resume_recorded(store, id, setup, config, None).await
}

/// Like [`resume_game_with`], writing the game's milestones to `recorder`.
pub(crate) async fn resume_recorded<S: EventStore>(
    store: S,
    id: GameId,
    setup: GameSetup,
    config: GameConfig,
    recorder: Option<Recorder>,
) -> Result<GameHandle, StartError> {
    let stored = store.load(id).await?;
    if stored.is_empty() {
        return spawn_recorded(id, setup, store, config, recorder);
    }
    let stored_len = stored.len();
    let engine = Engine::resume(setup.clone(), stored)?;
    // Rebuilding can run past the last stored decision; keep the store in step with the engine.
    let fresh = &engine.events()[stored_len..];
    if !fresh.is_empty() {
        store.append(id, count(stored_len), fresh).await?;
    }
    let mut running = Running::new(engine, config.pace);
    running.settle(false);
    Ok(launch(
        id,
        setup,
        store,
        config,
        Phase::Running(Box::new(running)),
        recorder,
    ))
}

fn launch<S: EventStore>(
    id: GameId,
    setup: GameSetup,
    store: S,
    config: GameConfig,
    phase: Phase,
    recorder: Option<Recorder>,
) -> GameHandle {
    let (commands, inbox) = mpsc::channel(config.command_queue.max(1));
    let actor = GameActor {
        id,
        store,
        seats: Seats::new(setup.players.len(), config.seat_queue),
        previews: PreviewLimits::new(setup.players.len(), Instant::now()),
        auto_pass: vec![config.pace.auto_pass; setup.players.len()],
        pace: config.pace,
        setup,
        phase,
        recorder,
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
    /// How fast each seat may ask for previews.
    pub(crate) previews: PreviewLimits,
    /// How fast the game moves (settings shared by every seat).
    pub(crate) pace: Pace,
    /// Per seat: whether a priority decision it cannot use is passed for it. Starts at the server default, resets on resume.
    pub(crate) auto_pass: Vec<bool>,
    pub(crate) phase: Phase,
    /// Where a persistent game notes its milestones; `None` for a game that lives in memory only.
    pub(crate) recorder: Option<Recorder>,
}

impl<S: EventStore> GameActor<S> {
    /// Handles commands one at a time until every handle is gone, and answers for a seat whose clock runs out.
    async fn run(mut self, mut inbox: mpsc::Receiver<Command>) {
        if self.start_up().await.is_err() {
            return;
        }
        loop {
            let command = match self.due() {
                None => inbox.recv().await,
                // Biased: a command already waiting was sent before the clock ran out, so it goes first.
                Some(due) => tokio::select! {
                    biased;
                    command = inbox.recv() => command,
                    () = tokio::time::sleep_until(due) => {
                        if self.expire().await.is_err() {
                            return;
                        }
                        continue;
                    }
                },
            };
            let Some(command) = command else {
                return;
            };
            if self.handle(command).await.is_err() {
                return;
            }
        }
    }

    async fn handle(&mut self, command: Command) -> Result<(), Fatal> {
        match command {
            Command::Join { seat, reply } => self.join(seat, reply).await,
            Command::Message { seat, message } => return self.message(seat, message).await,
            Command::Leave { seat } => self.seats.detach(seat),
            Command::Status { reply } => {
                // A caller that stopped waiting needs no answer.
                let _ = reply.send(self.status());
            }
        }
        Ok(())
    }

    async fn join(
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
            auto_pass: self.auto_pass.get(seat.index()).copied().unwrap_or(false),
        };
        self.seats.send(seat, welcome);
        self.sync(seat);
        let _ = reply.send(Ok(inbox));
        // After the reply, so the client never waits for the database.
        if let Some(recorder) = &mut self.recorder {
            recorder.joined(seat).await;
        }
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
            ClientMessage::Preview { decision, answer } => self.preview(seat, decision, &answer),
            ClientMessage::Concede => return self.concede(seat).await,
            ClientMessage::AutoPass { enabled } => return self.set_auto_pass(seat, enabled).await,
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
