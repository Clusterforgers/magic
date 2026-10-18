//! The cheap-clone front door to a running game actor.

use mtg_core::ids::PlayerId;
use mtg_engine::state::GameResult;
use mtg_protocol::{ClientMessage, ServerMessage};
use thiserror::Error;
use tokio::sync::{mpsc, oneshot};

use crate::game_id::GameId;

/// Why a seat could not be joined.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum JoinError {
    /// The game has no such seat.
    #[error("the game has no seat {0:?}")]
    NoSuchSeat(PlayerId),
    /// The game's task has ended.
    #[error("the game is closed")]
    Closed,
}

/// The game's task has ended, so nothing more can be sent to it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[error("the game is closed")]
pub struct GameClosed;

/// A snapshot of the actor's own bookkeeping, for tests, health checks and admin views.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GameStatus {
    /// Whether the engine exists yet (the lobby is over).
    pub started: bool,
    /// Events in the game's log.
    pub events: u64,
    /// Fingerprint of the game state; `None` before the start.
    pub state_hash: Option<u64>,
    /// The seat that must answer next, if any.
    pub waiting_on: Option<PlayerId>,
    /// The result once the game is over.
    pub result: Option<GameResult>,
    /// Seats with a live connection, in seat order.
    pub connected: Vec<PlayerId>,
    /// Seats that pressed Ready and did not take it back, in seat order; every seat once the game runs.
    pub ready: Vec<PlayerId>,
}

/// What a handle asks of the actor.
#[derive(Debug)]
pub(crate) enum Command {
    Join {
        seat: PlayerId,
        reply: oneshot::Sender<Result<mpsc::Receiver<ServerMessage>, JoinError>>,
    },
    Message {
        seat: PlayerId,
        message: ClientMessage,
    },
    Leave {
        seat: PlayerId,
    },
    Status {
        reply: oneshot::Sender<GameStatus>,
    },
}

/// A cheap-to-clone reference to one game; the game lives while any handle does.
#[derive(Debug, Clone)]
pub struct GameHandle {
    id: GameId,
    commands: mpsc::Sender<Command>,
}

impl GameHandle {
    pub(crate) fn new(id: GameId, commands: mpsc::Sender<Command>) -> GameHandle {
        GameHandle { id, commands }
    }

    /// The game this handle talks to.
    pub fn id(&self) -> GameId {
        self.id
    }

    /// Connects `seat`, replacing any earlier connection, and returns its queue of server messages.
    ///
    /// The queue starts with `Welcome`, then (once the game runs) a snapshot and the seat's pending
    /// decision. It is bounded: a client that stops reading is dropped and must join again.
    pub async fn join(&self, seat: PlayerId) -> Result<mpsc::Receiver<ServerMessage>, JoinError> {
        let (reply, answer) = oneshot::channel();
        self.commands
            .send(Command::Join { seat, reply })
            .await
            .map_err(|_| JoinError::Closed)?;
        answer.await.map_err(|_| JoinError::Closed)?
    }

    /// Delivers a client message on behalf of `seat`; replies arrive on the seat's queue.
    pub async fn send(&self, seat: PlayerId, message: ClientMessage) -> Result<(), GameClosed> {
        self.commands
            .send(Command::Message { seat, message })
            .await
            .map_err(|_| GameClosed)
    }

    /// Disconnects `seat`; the game waits for it to return.
    ///
    /// This closes whichever connection the seat has now, so a client that was already replaced
    /// by a newer join should simply drop its queue instead.
    pub async fn leave(&self, seat: PlayerId) {
        // A closed game has no connections left to drop, so there is nothing to report.
        let _ = self.commands.send(Command::Leave { seat }).await;
    }

    /// The actor's view of the game right now.
    pub async fn status(&self) -> Result<GameStatus, GameClosed> {
        let (reply, answer) = oneshot::channel();
        self.commands
            .send(Command::Status { reply })
            .await
            .map_err(|_| GameClosed)?;
        answer.await.map_err(|_| GameClosed)
    }
}
