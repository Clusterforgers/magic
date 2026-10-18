//! The live games: creation with per-seat tokens, the cap on concurrent games, and the janitor.
//!
//! A registry made with [`Registry::with_database`] also records every game it creates (recipe,
//! seats, status) and brings the unfinished ones back with [`Registry::resume`]; one made with
//! [`Registry::new`] keeps everything in memory.

mod resume;

use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::Instant;

use mtg_core::ids::PlayerId;
use mtg_engine::GameSetup;
use mtg_engine::state::GameResult;
use sqlx::PgPool;
use thiserror::Error;
use tokio::sync::watch;
use tokio::task::JoinHandle;

use crate::actor::{GameConfig, StartError, spawn_game_with, spawn_recorded};
use crate::config::{Retention, ServerConfig};
use crate::demo::DemoDeck;
use crate::game_id::GameId;
use crate::handle::{GameHandle, GameStatus};
use crate::recipe::{DeckRef, GameRecipe, RecipeError};
use crate::recorder::Recorder;
use crate::records::{GameRecords, NewSeat, RecordError};
use crate::secret::{RandomError, SeatToken, TokenDigest, looks_like_token, random_seed};
use crate::store::MemoryStore;

pub use resume::ResumeReport;

/// Why a game could not be created.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum CreateError {
    /// The server already runs its maximum number of games.
    #[error("the server is running its maximum number of games")]
    Full,
    /// The operating system could not supply random bytes.
    #[error(transparent)]
    Random(#[from] RandomError),
    /// The actor refused the setup.
    #[error(transparent)]
    Start(#[from] StartError),
    /// The requested decks and seats do not make a playable game.
    #[error(transparent)]
    Recipe(#[from] RecipeError),
    /// The game could not be recorded, so it was not started.
    #[error(transparent)]
    Record(#[from] RecordError),
}

/// One seat of a game to create.
#[derive(Debug, Clone)]
pub struct SeatRequest {
    /// Display name, already validated by the caller.
    pub name: String,
    /// The deck the seat plays.
    pub deck: &'static DemoDeck,
}

/// A new game and the secret credential of each seat.
#[derive(Debug)]
pub struct Created {
    /// The game.
    pub id: GameId,
    /// Every seat with its token, in seat order. This is the only time the tokens exist in clear.
    pub seats: Vec<(PlayerId, SeatToken)>,
}

/// A seat as everybody may see it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SeatSummary {
    /// The seat.
    pub seat: PlayerId,
    /// Its display name.
    pub name: String,
}

/// What anybody may learn about a game: no tokens, no cards, no seed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PublicGame {
    /// Whether every seat is ready and the game began.
    pub started: bool,
    /// The seats and their names.
    pub seats: Vec<SeatSummary>,
    /// Seats with a live connection.
    pub connected: Vec<PlayerId>,
    /// How the game ended, once it did.
    pub result: Option<GameResult>,
}

/// What a valid token opens: one seat of one game.
#[derive(Debug, Clone)]
pub struct SeatAccess {
    /// The game.
    pub game: GameId,
    /// Its actor.
    pub handle: GameHandle,
    /// The only seat the token grants.
    pub seat: PlayerId,
    /// Reports an error once the registry drops the game.
    pub removed: watch::Receiver<()>,
}

struct Entry {
    handle: GameHandle,
    names: Vec<String>,
    digests: Vec<TokenDigest>,
    created: Instant,
    /// When a sweep first saw the game finished.
    finished: Option<Instant>,
    /// Last time a sweep saw a seat connected.
    last_connected: Instant,
    /// Dropped with the entry, which tells every socket of the game to close.
    alive: watch::Sender<()>,
}

impl Entry {
    /// An entry for a game created at `created` that nobody has connected to yet.
    fn new(
        handle: GameHandle,
        names: Vec<String>,
        digests: Vec<TokenDigest>,
        created: Instant,
    ) -> Entry {
        let (alive, _) = watch::channel(());
        Entry {
            handle,
            names,
            digests,
            created,
            finished: None,
            last_connected: Instant::now(),
            alive,
        }
    }

    /// Updates the entry from a fresh status; false once the game is due for removal.
    fn keep(&mut self, status: &GameStatus, now: Instant, rules: &Retention) -> bool {
        let age = |since: Instant| now.saturating_duration_since(since);
        if status.result.is_some() {
            let since = *self.finished.get_or_insert(now);
            return age(since) < rules.finished_after;
        }
        if !status.started {
            return age(self.created) < rules.lobby_after;
        }
        if !status.connected.is_empty() {
            self.last_connected = now;
        }
        age(self.last_connected) < rules.abandoned_after
    }
}

struct Shared {
    games: Mutex<HashMap<GameId, Entry>>,
    /// Held while a game is created, so the cap holds even when recording takes a while.
    creating: tokio::sync::Mutex<()>,
    max_games: usize,
    fixed_seed: Option<u64>,
    game: GameConfig,
    retention: Retention,
    /// Where games are recorded; `None` keeps everything in memory.
    records: Option<GameRecords>,
}

/// The games of one server, by id. Cheap to clone.
#[derive(Clone)]
pub struct Registry {
    shared: Arc<Shared>,
}

impl Registry {
    /// An empty registry following `config`; games live in memory and end with the process.
    pub fn new(config: &ServerConfig) -> Registry {
        Registry::build(config, None)
    }

    /// An empty registry that records its games in `pool` (already migrated). Call
    /// [`Registry::resume`] to bring back the games an earlier process left unfinished.
    pub fn with_database(config: &ServerConfig, pool: PgPool) -> Registry {
        Registry::build(config, Some(GameRecords::new(pool)))
    }

    fn build(config: &ServerConfig, records: Option<GameRecords>) -> Registry {
        Registry {
            shared: Arc::new(Shared {
                games: Mutex::new(HashMap::new()),
                creating: tokio::sync::Mutex::new(()),
                max_games: config.max_games,
                fixed_seed: config.fixed_seed,
                game: config.game,
                retention: config.retention,
                records,
            }),
        }
    }

    fn games(&self) -> MutexGuard<'_, HashMap<GameId, Entry>> {
        // Nothing here can leave the map half-updated, so a poisoned lock is still sound.
        self.shared
            .games
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
    }

    /// Number of games alive.
    pub fn len(&self) -> usize {
        self.games().len()
    }

    /// Whether no game is alive.
    pub fn is_empty(&self) -> bool {
        self.games().is_empty()
    }

    /// Creates a Normal game with one seat per request; the seed is server-chosen.
    ///
    /// With a database the game is recorded (recipe, seats, token digests) before it exists, so a
    /// failed write creates nothing. Needs a tokio runtime.
    pub async fn create(&self, seats: &[SeatRequest]) -> Result<Created, CreateError> {
        let seed = match self.shared.fixed_seed {
            Some(seed) => seed,
            None => random_seed()?,
        };
        let decks = seats.iter().map(|seat| DeckRef::Demo {
            id: seat.deck.id.to_owned(),
        });
        let recipe = GameRecipe::normal(seed, decks);
        let setup = recipe.build()?;
        let names = seats.iter().map(|s| s.name.clone()).collect();
        self.admit(setup, names, Some(recipe)).await
    }

    /// Registers a game built by the caller (a missing name becomes "Seat N"). Needs a tokio runtime.
    ///
    /// Such a game has no recipe, so it is never recorded and does not survive a restart, even on a
    /// registry with a database. Meant for tests that need a setup no request can describe.
    pub async fn register(
        &self,
        setup: GameSetup,
        names: Vec<String>,
    ) -> Result<Created, CreateError> {
        self.admit(setup, names, None).await
    }

    /// Gives a validated setup its tokens, records it when it has a recipe and starts its actor.
    async fn admit(
        &self,
        setup: GameSetup,
        mut names: Vec<String>,
        recipe: Option<GameRecipe>,
    ) -> Result<Created, CreateError> {
        let seats = setup.players.len();
        names.truncate(seats);
        while names.len() < seats {
            names.push(format!("Seat {}", names.len() + 1));
        }
        let tokens = (0..seats)
            .map(|_| SeatToken::generate())
            .collect::<Result<Vec<_>, _>>()?;
        let digests: Vec<TokenDigest> = tokens.iter().map(SeatToken::digest).collect();
        // Before anything is written, so a setup the actor would refuse leaves no record.
        setup.validate().map_err(StartError::Setup)?;

        let _creating = self.shared.creating.lock().await;
        let id = {
            let games = self.games();
            if games.len() >= self.shared.max_games {
                return Err(CreateError::Full);
            }
            loop {
                let id = GameId::random();
                if !games.contains_key(&id) {
                    break id;
                }
            }
        };
        let handle = match (&self.shared.records, &recipe) {
            (Some(records), Some(recipe)) => {
                let rows: Vec<NewSeat> = names
                    .iter()
                    .zip(&digests)
                    .map(|(name, digest)| NewSeat {
                        name: name.clone(),
                        digest: *digest.as_bytes(),
                    })
                    .collect();
                records.insert(id, recipe, &rows).await?;
                let recorder = Recorder::new(records.clone(), id, vec![false; seats]);
                let spawned =
                    spawn_recorded(id, setup, records.store(), self.shared.game, Some(recorder));
                match spawned {
                    Ok(handle) => handle,
                    Err(error) => {
                        // Not reachable after `validate`, but a record must not outlive its game.
                        let _ = records.mark_failed(id, &error.to_string()).await;
                        return Err(error.into());
                    }
                }
            }
            _ => spawn_game_with(id, setup, MemoryStore::new(), self.shared.game)?,
        };
        self.games()
            .insert(id, Entry::new(handle, names, digests, Instant::now()));
        let seats = tokens
            .into_iter()
            .enumerate()
            .map(|(seat, token)| (PlayerId::from_index(seat), token))
            .collect();
        Ok(Created { id, seats })
    }

    /// The actor of a game, for tests and admin views.
    pub fn handle(&self, id: GameId) -> Option<GameHandle> {
        self.games().get(&id).map(|entry| entry.handle.clone())
    }

    /// Maps a presented token to its seat; `None` for an unknown game and a wrong token alike.
    pub fn authenticate(&self, id: GameId, token: &str) -> Option<SeatAccess> {
        if !looks_like_token(token) {
            return None;
        }
        let presented = TokenDigest::of(token);
        let games = self.games();
        let entry = games.get(&id)?;
        // Check every seat so the time taken does not say which one matched.
        let mut found = None;
        for (seat, digest) in entry.digests.iter().enumerate() {
            if digest.matches(&presented) {
                found = Some(seat);
            }
        }
        Some(SeatAccess {
            game: id,
            handle: entry.handle.clone(),
            seat: PlayerId::from_index(found?),
            removed: entry.alive.subscribe(),
        })
    }

    /// The public facts of a game; `None` if there is no such game or its actor is gone.
    pub async fn describe(&self, id: GameId) -> Option<PublicGame> {
        let (handle, names) = {
            let games = self.games();
            let entry = games.get(&id)?;
            (entry.handle.clone(), entry.names.clone())
        };
        let status = handle.status().await.ok()?;
        Some(PublicGame {
            started: status.started,
            seats: names
                .into_iter()
                .enumerate()
                .map(|(seat, name)| SeatSummary {
                    seat: PlayerId::from_index(seat),
                    name,
                })
                .collect(),
            connected: status.connected,
            result: status.result,
        })
    }

    /// Drops what has outlived its retention at time `now`; returns how many games went.
    pub async fn sweep(&self, now: Instant) -> usize {
        let handles: Vec<(GameId, GameHandle)> = self
            .games()
            .iter()
            .map(|(id, entry)| (*id, entry.handle.clone()))
            .collect();
        let mut statuses = Vec::with_capacity(handles.len());
        for (id, handle) in handles {
            statuses.push((id, handle.status().await.ok()));
        }
        let mut dropped = Vec::new();
        {
            let mut games = self.games();
            for (id, status) in statuses {
                let Some(entry) = games.get_mut(&id) else {
                    continue;
                };
                // A game whose actor stopped can never be played again.
                let keep = status
                    .as_ref()
                    .is_some_and(|s| entry.keep(s, now, &self.shared.retention));
                if !keep {
                    games.remove(&id);
                    dropped.push((id, status.and_then(|s| s.result)));
                }
            }
        }
        if let Some(records) = &self.shared.records {
            // A dropped game must not come back on the next boot. A finished one is noted again in
            // case its own write failed; both updates leave games that already ended alone.
            for (id, result) in &dropped {
                let noted = match result {
                    Some(result) => records.mark_finished(*id, *result).await,
                    None => records.mark_expired(*id).await,
                };
                if let Err(error) = noted {
                    tracing::warn!(game = %id, %error, "could not record a dropped game");
                }
            }
        }
        dropped.len()
    }

    /// Runs [`Registry::sweep`] every `sweep_every` until `stop` fires or its sender is dropped.
    pub fn spawn_janitor(&self, mut stop: watch::Receiver<bool>) -> JoinHandle<()> {
        let registry = self.clone();
        let every = self
            .shared
            .retention
            .sweep_every
            .max(std::time::Duration::from_millis(1));
        tokio::spawn(async move {
            let mut ticker = tokio::time::interval(every);
            ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
            loop {
                tokio::select! {
                    _ = ticker.tick() => {
                        let dropped = registry.sweep(Instant::now()).await;
                        if dropped > 0 {
                            tracing::info!(dropped, alive = registry.len(), "janitor dropped games");
                        }
                    }
                    _ = stop.changed() => break,
                }
            }
        })
    }
}
