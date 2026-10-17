//! Game server core: one actor per game, fed through cheap-clone handles (no network yet).
//!
//! [`spawn_game`] starts a game in its lobby and [`resume_game`] rebuilds a running one from an
//! [`EventStore`]. Either returns a [`GameHandle`]; a transport joins a seat, forwards the client's
//! [`ClientMessage`](mtg_protocol::ClientMessage)s and relays what arrives on the seat's queue.
//! Every state change is saved before it is shown, and nothing leaves the actor except
//! per-viewer projections from `mtg-protocol`.

mod actor;
mod game_id;
mod handle;
mod lobby;
mod play;
mod running;
mod seats;
mod store;

pub use actor::{
    GameConfig, MAX_CHAT_CHARS, StartError, resume_game, resume_game_with, spawn_game,
    spawn_game_with,
};
pub use game_id::GameId;
pub use handle::{GameClosed, GameHandle, GameStatus, JoinError};
pub use seats::MIN_QUEUE;
pub use store::{EventStore, MemoryStore, StoreError};
