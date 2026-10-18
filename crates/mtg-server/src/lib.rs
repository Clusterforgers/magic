//! Game server: one actor per game, fed through cheap-clone handles, behind an axum transport.
//!
//! [`spawn_game`] starts a game in its lobby and [`resume_game`] rebuilds a running one from an
//! [`EventStore`]. Either returns a [`GameHandle`]; the transport ([`Server`]) joins a seat,
//! forwards the client's [`ClientMessage`](mtg_protocol::ClientMessage)s and relays what arrives on
//! the seat's queue. The [`Registry`] owns the live games and the per-seat tokens.
//!
//! How a game behaves:
//! - **Lobby.** Until every seat has sent `Ready` there is no engine; joins, readiness and chat
//!   are all the actor handles. The last `Ready` creates the engine, saves its opening events and
//!   shows them to every seat from seq 0 like any other batch. If that fails the lobby stays as it
//!   was and a repeated `Ready` retries. A game resumed from a non-empty log goes straight to
//!   running; one with nothing stored never started and returns to its lobby.
//! - **Persist first.** A step (an answer or a concession) is applied to the engine, projected for
//!   the connected seats, saved, and only then sent. If the save fails the engine is rolled back
//!   (`Engine::restore`), the sender gets `Error{Internal}` and nobody else hears of it.
//! - **Decisions.** Each decision gets a fresh [`DecisionId`](mtg_protocol::DecisionId) and goes
//!   only to the seat it names. Answers from another seat, with another id, or after the decision
//!   was answered get `StaleDecision`; answers the engine rejects get `IllegalAnswer`; neither
//!   changes the game. A concession that leaves the pending decision untouched keeps its id.
//! - **Connections.** `join` replaces whatever connection the seat had and sends `Welcome`, then
//!   (once running) a snapshot, the seat's pending decision and the result if the game is over;
//!   `Resync` repeats the last two. Queues are bounded: a client that stops reading is dropped and
//!   must join again. The game waits for absent seats and never passes for them.
//! - **Chat** is broadcast to connected seats and never stored.
//!
//! Event logs live in memory ([`MemoryStore`]) or in Postgres ([`PgEventStore`], whose schema
//! [`migrate`] applies and tracks in its own table so `mtg-db` can share the database).
//!
//! With a database ([`Server::open`]) every game the REST path creates is also recorded: a
//! [`GameRecipe`] that rebuilds its `GameSetup`, its seats with the SHA-256 of each token, and its
//! status ([`RecordStatus`]). On boot [`Registry::resume`] brings back every game that is not
//! finished, so a client that returns with its old token lands in the same game.

mod actor;
mod api;
pub mod config;
pub mod demo;
mod game_id;
mod handle;
mod limit;
mod lobby;
mod migrations;
mod origin;
mod pg_store;
mod play;
mod recipe;
mod recorder;
mod records;
pub mod registry;
mod running;
mod seats;
mod secret;
mod server;
mod session;
mod store;
mod ws;

pub use actor::{
    GameConfig, MAX_CHAT_CHARS, StartError, resume_game, resume_game_with, spawn_game,
    spawn_game_with,
};
pub use api::{BODY_LIMIT, MAX_NAME_CHARS, MAX_SEATS, MIN_SEATS};
pub use config::{ConfigError, ServerConfig};
pub use game_id::GameId;
pub use handle::{GameClosed, GameHandle, GameStatus, JoinError};
pub use migrations::{MIGRATIONS_TABLE, migrate, migrator};
pub use pg_store::PgEventStore;
pub use recipe::{DeckRef, FormatKind, GameRecipe, RecipeError};
pub use records::{GameRecord, GameRecords, RecordError, RecordStatus, SeatRecord};
pub use registry::{Registry, ResumeReport};
pub use seats::MIN_QUEUE;
pub use secret::{RandomError, SeatToken};
pub use server::Server;
pub use store::{EventStore, MemoryStore, StoreError};
