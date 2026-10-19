//! Game server: one actor per game, fed through cheap-clone handles, behind an axum transport.
//!
//! [`spawn_game`] starts a game in its lobby and [`resume_game`] rebuilds a running one from an
//! [`EventStore`]. Either returns a [`GameHandle`]; the transport ([`Server`]) joins a seat,
//! forwards the client's [`ClientMessage`](mtg_protocol::ClientMessage)s and relays what arrives on
//! the seat's queue. The [`Registry`] owns the live games and the per-seat tokens.
//!
//! How a game behaves:
//! - **Lobby.** Until every seat has sent `Ready` there is no engine; joins, readiness (`Ready`,
//!   and `NotReady` to take it back) and chat are all the actor handles. Every connected seat is
//!   kept told who is at the table (`ServerMessage::Lobby`: names, who is connected, who is ready)
//!   after its `Welcome` and at each change. The last `Ready` creates the engine, saves its opening
//!   events and shows them to every seat from seq 0 like any other batch. If that fails the lobby
//!   stays as it was and a repeated `Ready` retries. A game resumed from a non-empty log goes
//!   straight to running; one with nothing stored never started and returns to its lobby, where
//!   nobody is ready (readiness is not stored).
//! - **Persist first.** A step (an answer or a concession) is applied to the engine, projected for
//!   the connected seats, saved, and only then sent. If the save fails the engine is rolled back
//!   (`Engine::restore`), the sender gets `Error{Internal}` and nobody else hears of it.
//! - **Decisions.** Each decision gets a fresh [`DecisionId`](mtg_protocol::DecisionId) and goes
//!   only to the seat it names. Answers from another seat, with another id, or after the decision
//!   was answered get `StaleDecision`; answers the engine rejects get `IllegalAnswer`; neither
//!   changes the game. A concession that leaves the pending decision untouched keeps its id.
//! - **Connections.** `join` replaces whatever connection the seat had and sends `Welcome`, then
//!   the lobby roster, or (once running) a snapshot, the seat's pending decision and the result if
//!   the game is over;
//!   `Resync` repeats the last two. Queues are bounded: a client that stops reading is dropped and
//!   must join again. The game waits for absent seats, with the exceptions of the next bullet.
//! - **Pace.** A seat is asked for priority only at its windows (`windows.rs`): its own main phases, the
//!   declare attackers and declare blockers steps and any moment an object of another seat is on the stack
//!   (each when the seat can act), and the steps where it holds a stop (`ClientMessage::SetStop`). Anywhere
//!   else it is passed for ([`Pace`], `ClientMessage::AutoPass`) through the same path as an answer, and its
//!   client is never sent that decision. At a window of the table that it cannot use (a combat declaration, a
//!   spell on the stack; not its own main phase with an empty stack), the pass waits a short dwell first
//!   (`Pace::dwell`, `MTG_DWELL_MS`) and is a step of its own, so the pace does not show what the seat holds. A seat in Full Control (`ClientMessage::FullControl`; turning
//!   auto-pass off is the same as `Held`) is asked at every window. A seat that yields is passed for as well
//!   (`ClientMessage::Yield`: until the stack is empty, the turn ends or its own turn begins; an
//!   opponent's new object, whatever the seat could do about it, or a stop of its own ends it). Stops, modes
//!   and yields are held in memory per seat and reported back.
//!   A seat that is not the active player has a clock on every priority decision (10 s) and on declaring
//!   blockers (45 s); at the deadline the server answers for the seat (a pass, or no blocks) the same
//!   way, so a late answer is stale. Every other decision waits as long as it takes.
//! - **Chat** is broadcast to connected seats and never stored.
//! - **Previews** (`ClientMessage::Preview`) are advice about a pending attack or block: the actor answers
//!   each with one `Forecast` (empty when there is none or the seat is over its rate limit), never an
//!   error, and writes nothing.
//!
//! Event logs live in memory ([`MemoryStore`]) or in Postgres ([`PgEventStore`], whose schema
//! [`migrate`] applies and tracks in its own table so `mtg-db` can share the database).
//!
//! With a database ([`Server::open`]) every game the REST path creates is also recorded: a
//! [`GameRecipe`] that rebuilds its `GameSetup` (and fingerprints each seat's deck, so a deck that
//! a deploy changed fails the game instead of replaying its log under other cards), its seats with
//! the SHA-256 of each token, and its status ([`RecordStatus`]). On boot [`Registry::resume`]
//! brings back every game that is not finished, so a client that returns with its old token lands
//! in the same game. A recipe can name a deck by the oracle ids of its cards: [`CardSource`]
//! assembles them from the card database and the rules files, and the same fingerprint covers the
//! result.
//!
//! The same database holds the accounts: usernames with Argon2id password hashes and browser
//! sessions kept as the SHA-256 of the `mtg_session` cookie's token (`/api/auth/*`). A seat can
//! belong to an account ([`Registry::bind_seat`]): its token keeps opening it, and the account
//! adds a socket without the token (`/ws/{game}` with the cookie) and a list of its games
//! (`/api/me/games`). Without a database there are no accounts and every game is guest-only.

mod accounts;
mod actor;
mod api;
mod auth;
mod card_api;
mod cards;
pub mod config;
mod cookie;
mod credentials;
mod deck_cards;
mod deck_import;
mod decks;
pub mod demo;
mod dwell;
mod game_id;
mod handle;
mod limit;
mod lobby;
mod migrations;
mod my_games;
mod origin;
mod pace;
mod passwords;
mod pg_store;
mod play;
mod preview;
mod recipe;
mod recorder;
mod records;
pub mod registry;
mod running;
mod seats;
mod secret;
mod server;
mod session;
mod stops;
mod store;
mod throttle;
mod user_id;
mod windows;
mod ws;
mod yields;

pub use actor::{
    GameConfig, MAX_CHAT_CHARS, StartError, resume_game, resume_game_with, spawn_game,
    spawn_game_with,
};
pub use api::{BODY_LIMIT, MAX_NAME_CHARS, MAX_SEATS, MIN_SEATS};
pub use cards::{CardSource, CatalogError, printed_card};
pub use config::{AuthConfig, ConfigError, ServerConfig};
pub use dwell::{Dwell, MAX_DWELL_MS};
pub use game_id::GameId;
pub use handle::{GameClosed, GameHandle, GameStatus, JoinError};
pub use migrations::{MIGRATIONS_TABLE, migrate, migrator};
pub use pace::{MAX_CLOCK_SECS, Pace};
pub use passwords::{HashCost, HashCostError};
pub use pg_store::PgEventStore;
pub use preview::{PREVIEW_BURST, PREVIEWS_PER_SECOND};
pub use recipe::{
    Catalog, DeckFingerprint, DeckRef, FINGERPRINT_VERSION, FingerprintError, FormatKind,
    GameRecipe, RecipeError,
};
pub use records::{
    Binding, GameRecord, GameRecords, RecordError, RecordStatus, SeatRecord, UserGame, UserGameSeat,
};
pub use registry::{Registry, ResumeReport};
pub use seats::MIN_QUEUE;
pub use secret::{RandomError, SeatToken};
pub use server::Server;
pub use store::{EventStore, MemoryStore, StoreError};
pub use user_id::UserId;
