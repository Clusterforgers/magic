//! The live games: creation with per-seat tokens, the cap on concurrent games, and the janitor.
//!
//! A registry made with [`Registry::with_database`] also records every game it creates (recipe,
//! seats, status) and brings the unfinished ones back with [`Registry::resume`]; one made with
//! [`Registry::new`] keeps everything in memory.
//!
//! A seat can belong to an account. The token still opens the seat; the account is a second way
//! in ([`Registry::authenticate_user`]) and puts the game on the account's list
//! ([`Registry::games_of`]). A seat is bound once, to the first account that claims it
//! ([`Registry::bind_seat`]), and an account holds one seat per game.
//!
//! A game is also made [`Visibility::Public`] or [`Visibility::Invite`], and each of its seats is
//! held from the start (its token is in the create response) or open ([`SeatState`]).

mod cause;
mod claims;
mod departure;
mod listing;
mod resume;

use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::Instant;

use mtg_core::house::HouseRules;
use mtg_core::ids::PlayerId;
use mtg_engine::GameSetup;
use mtg_engine::state::GameResult;
use sqlx::PgPool;
use thiserror::Error;
use tokio::sync::watch;
use tokio::task::JoinHandle;

use crate::actor::{GameConfig, StartError, spawn_named};
use crate::cards::{CardSource, CatalogError};
use crate::config::{Retention, ServerConfig};
use crate::demo::{self, DemoDeck};
use crate::game_id::GameId;
use crate::handle::{GameHandle, GameStatus};
use crate::join_code::JoinCode;
use crate::recipe::{Catalog, FormatKind, GameRecipe, RecipeError};
use crate::recorder::Recorder;
use crate::records::{
    Binding, GameRecords, NewGame, NewSeat, RecordError, SeatState, UserGame, Visibility,
};
use crate::seat_deck::SeatDecks;
use crate::secret::{RandomError, SeatToken, TokenDigest, looks_like_token, random_seed};
use crate::store::MemoryStore;
use crate::throttle::{Blocked, Throttle};
use crate::user_id::UserId;

/// Most games `Registry::games_of` returns: the newest ones.
pub const MAX_LISTED_GAMES: usize = 100;

pub use claims::{ClaimError, Entrance, HOST, SeatClaim, Seated, TableError};
pub use departure::Departure;
pub use listing::{ListedGame, MAX_LISTED_LOBBIES};
pub use resume::ResumeReport;

pub(crate) use cause::Cause;

pub use crate::seat_deck::ChosenDeck;

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
    /// The cards of a deck of oracle ids could not be had.
    #[error(transparent)]
    Cards(#[from] CatalogError),
}

/// One seat of a game to create.
#[derive(Debug, Clone)]
pub struct SeatRequest {
    /// Display name, already validated by the caller.
    pub name: String,
    /// The deck the seat plays.
    pub deck: &'static DemoDeck,
}

/// What a new game is made with besides its seats.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct GameOptions {
    /// Who may find and join the game; invite-only unless said otherwise.
    pub visibility: Visibility,
    /// How many seats, counted from seat 0, are held from the start: each gets its token in the
    /// create response and the names of the request. The rest are open for a join to claim, their
    /// requested names are ignored, and the decks are the ones requested. `None` holds them all,
    /// as [`Registry::create`] does. The creator plays seat 0, so at least that one is held.
    pub held: Option<usize>,
    /// What the creator sets in place of the format's values; a game made with [`Registry::create_decked`]
    /// plays under them, one made from a recipe under the recipe's.
    pub house_rules: HouseRules,
    /// The format of a game made with [`Registry::create_decked`]; a game made from a recipe plays
    /// the recipe's.
    pub format: FormatKind,
}

/// A new game and the secret credential of each seat that is held.
#[derive(Debug)]
pub struct Created {
    /// The game.
    pub id: GameId,
    /// Every held seat with its token, in seat order; open seats have none. This is the only time
    /// the tokens exist in clear.
    pub seats: Vec<(PlayerId, SeatToken)>,
    /// The code that lets anybody who has it into the game's open seats. Like the tokens, it
    /// exists in clear only here.
    pub code: JoinCode,
}

/// A seat as everybody may see it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SeatSummary {
    /// The seat.
    pub seat: PlayerId,
    /// Its display name; empty while the seat is open.
    pub name: String,
    /// How the seat is held: its token was made with the game, a join claimed it, or a join may.
    pub state: SeatState,
}

/// What anybody may learn about a game: no tokens, no cards, no seed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PublicGame {
    /// Who may find and join the game.
    pub visibility: Visibility,
    /// Whether every seat is ready and the game began.
    pub started: bool,
    /// The seats and their names.
    pub seats: Vec<SeatSummary>,
    /// Seats with a live connection.
    pub connected: Vec<PlayerId>,
    /// Seats that pressed Ready and did not take it back; every seat once the game began.
    pub ready: Vec<PlayerId>,
    /// How the game ended, once it did.
    pub result: Option<GameResult>,
}

/// Why a game left the registry, as the sockets of its seats learn it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Removal {
    /// The janitor dropped the game, or its actor ended.
    Dropped,
    /// The host closed the game while it was in its lobby.
    ClosedByHost,
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
    /// Reports an error, or a change, once the registry drops the game; the value says why.
    pub removed: watch::Receiver<Removal>,
}

/// What the registry knows about one seat.
#[derive(Debug, Clone)]
struct SeatSlot {
    state: SeatState,
    /// What the lobby calls the seat; empty while it is open.
    name: String,
    /// The token's digest; none while the seat is open.
    digest: Option<TokenDigest>,
    /// The account the seat is bound to; guests' seats have none.
    user: Option<UserId>,
}

impl SeatSlot {
    /// A seat whose token was made with the game.
    fn issued(name: String, digest: TokenDigest, user: Option<UserId>) -> SeatSlot {
        SeatSlot {
            state: SeatState::Issued,
            name,
            digest: Some(digest),
            user,
        }
    }

    /// A seat nobody holds.
    fn open() -> SeatSlot {
        SeatSlot {
            state: SeatState::Open,
            name: String::new(),
            digest: None,
            user: None,
        }
    }
}

/// The slots of a new game with `seats` seats: a seat with a token is issued under its name (a
/// missing or blank name becomes "Seat N", since an empty name marks an open seat) and `host`
/// owns seat 0; the others are open.
fn seat_slots(
    mut names: Vec<String>,
    tokens: &[SeatToken],
    seats: usize,
    host: Option<UserId>,
) -> Vec<SeatSlot> {
    names.truncate(seats);
    while names.len() < seats {
        names.push(format!("Seat {}", names.len() + 1));
    }
    names
        .into_iter()
        .enumerate()
        .map(|(seat, name)| match tokens.get(seat) {
            Some(token) => {
                let name = if name.trim().is_empty() {
                    format!("Seat {}", seat + 1)
                } else {
                    name
                };
                SeatSlot::issued(name, token.digest(), host.filter(|_| seat == 0))
            }
            None => SeatSlot::open(),
        })
        .collect()
}

/// What an entry starts from.
struct Parts {
    visibility: Visibility,
    format: FormatKind,
    house_rules: HouseRules,
    code: Option<TokenDigest>,
    /// Whether the game is in the database, so changes to its seats are written there too.
    recorded: bool,
    seats: Vec<SeatSlot>,
}

struct Entry {
    handle: GameHandle,
    visibility: Visibility,
    format: FormatKind,
    /// What the creator set in place of the format's values.
    house_rules: HouseRules,
    /// The digest of the join code; none for a game stored before codes existed.
    code: Option<TokenDigest>,
    recorded: bool,
    /// Every seat in seat order.
    seats: Vec<SeatSlot>,
    /// Held while the seats of the game change hands, which spans the database and the actor, so
    /// two changes to one table never interleave.
    turn: Arc<tokio::sync::Mutex<()>>,
    created: Instant,
    /// When a sweep first saw the game finished.
    finished: Option<Instant>,
    /// Last time a sweep saw a seat connected.
    last_connected: Instant,
    /// Dropped with the entry, which tells every socket of the game to close; a game closed by its
    /// host says so first.
    alive: watch::Sender<Removal>,
}

impl Entry {
    /// An entry for a game created at `created` that nobody has connected to yet.
    fn new(handle: GameHandle, parts: Parts, created: Instant) -> Entry {
        let (alive, _) = watch::channel(Removal::Dropped);
        Entry {
            handle,
            visibility: parts.visibility,
            format: parts.format,
            house_rules: parts.house_rules,
            code: parts.code,
            recorded: parts.recorded,
            seats: parts.seats,
            turn: Arc::new(tokio::sync::Mutex::new(())),
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
    /// Where the cards of a deck of oracle ids come from; present with a database.
    cards: Option<CardSource>,
    /// Join code lookups that missed, per client address.
    throttle: Throttle,
}

/// The games of one server, by id. Cheap to clone.
#[derive(Clone)]
pub struct Registry {
    shared: Arc<Shared>,
}

impl Registry {
    /// An empty registry following `config`; games live in memory and end with the process.
    pub fn new(config: &ServerConfig) -> Registry {
        Registry::build(config, None, None)
    }

    /// An empty registry that records its games in `pool` (already migrated). Call
    /// [`Registry::resume`] to bring back the games an earlier process left unfinished. Decks of
    /// oracle ids are built from the `cards` table of the same database.
    pub fn with_database(config: &ServerConfig, pool: PgPool) -> Registry {
        let cards = CardSource::new(pool.clone());
        Registry::with_card_source(config, pool, cards)
    }

    /// Like [`Registry::with_database`], with the cards of decks of oracle ids coming from `cards`
    /// (other rules or capabilities than the ones of this build, for tests).
    pub fn with_card_source(config: &ServerConfig, pool: PgPool, cards: CardSource) -> Registry {
        Registry::build(config, Some(GameRecords::new(pool)), Some(cards))
    }

    fn build(
        config: &ServerConfig,
        records: Option<GameRecords>,
        cards: Option<CardSource>,
    ) -> Registry {
        Registry {
            shared: Arc::new(Shared {
                games: Mutex::new(HashMap::new()),
                creating: tokio::sync::Mutex::new(()),
                max_games: config.max_games,
                fixed_seed: config.fixed_seed,
                game: config.game,
                retention: config.retention,
                records,
                cards,
                throttle: Throttle::new(config.auth.limits),
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
        self.create_for(seats, None).await
    }

    /// Like [`Registry::create`] for a signed-in creator, who owns seat 0 from the start. A
    /// registry without a database has no accounts, so `host` is ignored there.
    pub async fn create_for(
        &self,
        seats: &[SeatRequest],
        host: Option<UserId>,
    ) -> Result<Created, CreateError> {
        self.create_with(seats, GameOptions::default(), host).await
    }

    /// Like [`Registry::create_for`] with the game's visibility and open seats from `options`.
    pub async fn create_with(
        &self,
        seats: &[SeatRequest],
        options: GameOptions,
        host: Option<UserId>,
    ) -> Result<Created, CreateError> {
        let names = seats.iter().map(|s| s.name.clone()).collect();
        let decks = seats
            .iter()
            .map(|seat| ChosenDeck::demo(seat.deck).map(Some))
            .collect::<Result<Vec<_>, _>>()
            .map_err(RecipeError::from)?;
        self.create_decked(names, decks, options, host).await
    }

    /// Creates a game of `options.format` whose seats start with `decks`, one entry per seat: the deck a seat
    /// plays until its holder chooses another, which is also the deck it returns to when a player
    /// gives it back. A seat with `None` has no deck and cannot press Ready until it chooses one;
    /// the recipe holds [`demo::stand_in`] for it meanwhile.
    ///
    /// The recipe names the decks as given, with the fingerprint of their cards, and the seed is
    /// server-chosen. A game made this way tells the table each seat's deck and holds back a `Ready`
    /// without one.
    pub async fn create_decked(
        &self,
        names: Vec<String>,
        decks: Vec<Option<ChosenDeck>>,
        options: GameOptions,
        host: Option<UserId>,
    ) -> Result<Created, CreateError> {
        let seed = match self.shared.fixed_seed {
            Some(seed) => seed,
            None => random_seed()?,
        };
        let stand_in = match options.format {
            FormatKind::Normal => ChosenDeck::demo(demo::stand_in()),
            FormatKind::Commander => ChosenDeck::commander_stand_in(),
        }
        .map_err(RecipeError::from)?;
        let held: Vec<&ChosenDeck> = decks
            .iter()
            .map(|deck| deck.as_ref().unwrap_or(&stand_in))
            .collect();
        // The decks as they are now, kept so that a later boot can tell when a deploy changed one.
        let recipe = GameRecipe {
            format: options.format,
            house_rules: options.house_rules.clone(),
            seed,
            decks: held.iter().map(|deck| deck.reference().clone()).collect(),
            fingerprints: held.iter().map(|deck| *deck.fingerprint()).collect(),
        };
        let libraries = held.iter().map(|deck| deck.cards().to_vec()).collect();
        let commanders = held.iter().map(|deck| deck.commanders().to_vec()).collect();
        let setup = GameSetup::new(options.format.rules(), seed, libraries)
            .with_commanders(commanders)
            .with_house_rules(options.house_rules.clone());
        // Refused like a recipe that does not build: too few seats, a deck the format does not allow.
        setup.validate().map_err(RecipeError::from)?;
        self.admit(setup, names, Some(recipe), host, options, Some(decks))
            .await
    }

    /// Creates a game from a recipe the caller made, whose decks may name cards by oracle id: their
    /// definitions are assembled from the card database first, the recipe takes the fingerprint of
    /// every deck as assembled now, and the game is recorded with it. The seed is the recipe's own.
    /// A registry without a database cannot have the cards of a deck of oracle ids.
    pub async fn create_from_recipe(
        &self,
        recipe: GameRecipe,
        names: Vec<String>,
        host: Option<UserId>,
    ) -> Result<Created, CreateError> {
        self.create_from_recipe_with(recipe, names, host, GameOptions::default())
            .await
    }

    /// Like [`Registry::create_from_recipe`] with the game's visibility and open seats from `options`.
    pub async fn create_from_recipe_with(
        &self,
        recipe: GameRecipe,
        names: Vec<String>,
        host: Option<UserId>,
        options: GameOptions,
    ) -> Result<Created, CreateError> {
        let catalog = match &self.shared.cards {
            Some(cards) => cards.catalog_for(&recipe).await?,
            None => Catalog::default(),
        };
        let recipe = recipe.with_fingerprints_from(&catalog)?;
        let setup = recipe.build_with(&catalog)?;
        self.admit(setup, names, Some(recipe), host, options, None)
            .await
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
        self.admit(setup, names, None, None, GameOptions::default(), None)
            .await
    }

    /// Gives a validated setup its tokens, records it when it has a recipe and starts its actor.
    /// `host` owns seat 0 when the game is recorded. `decks`, one entry per seat, makes the game
    /// one whose seats choose their decks in the lobby.
    async fn admit(
        &self,
        setup: GameSetup,
        names: Vec<String>,
        recipe: Option<GameRecipe>,
        host: Option<UserId>,
        options: GameOptions,
        decks: Option<Vec<Option<ChosenDeck>>>,
    ) -> Result<Created, CreateError> {
        let seats = setup.players.len();
        let held = options.held.map_or(seats, |held| held.max(1).min(seats));
        let tokens = (0..held)
            .map(|_| SeatToken::generate())
            .collect::<Result<Vec<_>, _>>()?;
        // Before anything is written, so a setup the actor would refuse leaves no record.
        setup.validate().map_err(StartError::Setup)?;

        let _creating = self.shared.creating.lock().await;
        let (id, code) = self.unused_id_and_code()?;
        // Bound only where it is recorded: a game without a database has no accounts to bind.
        let persistent = self.shared.records.is_some() && recipe.is_some();
        let host = host.filter(|_| persistent);
        let slots = seat_slots(names, &tokens, seats, host);
        let meta = NewGame {
            visibility: options.visibility,
            code_digest: Some(*code.digest().as_bytes()),
        };
        let house_rules = setup.house_rules.clone();
        let format = recipe
            .as_ref()
            .map_or(options.format, |recipe| recipe.format);
        let decks = decks.map(|defaults| {
            SeatDecks::new(format, setup.seed, defaults).with_house_rules(house_rules.clone())
        });
        let handle = self
            .start(id, setup, recipe.as_ref(), &meta, &slots, decks)
            .await?;
        let parts = Parts {
            visibility: options.visibility,
            format,
            house_rules,
            code: Some(code.digest()),
            recorded: persistent,
            seats: slots,
        };
        self.games()
            .insert(id, Entry::new(handle, parts, Instant::now()));
        let seats = tokens
            .into_iter()
            .enumerate()
            .map(|(seat, token)| (PlayerId::from_index(seat), token))
            .collect();
        Ok(Created { id, seats, code })
    }

    /// An id and a join code no live game has, if the cap leaves room for one more game.
    fn unused_id_and_code(&self) -> Result<(GameId, JoinCode), CreateError> {
        let games = self.games();
        if games.len() >= self.shared.max_games {
            return Err(CreateError::Full);
        }
        let id = loop {
            let id = GameId::random();
            if !games.contains_key(&id) {
                break id;
            }
        };
        // No two live games share a code, so a code names one game.
        let code = loop {
            let code = JoinCode::generate()?;
            let digest = code.digest();
            let taken = games
                .values()
                .any(|entry| entry.code.as_ref().is_some_and(|c| c.matches(&digest)));
            if !taken {
                break code;
            }
        };
        Ok((id, code))
    }

    /// Records the game when it has a recipe and a database, and starts its actor.
    async fn start(
        &self,
        id: GameId,
        setup: GameSetup,
        recipe: Option<&GameRecipe>,
        meta: &NewGame,
        slots: &[SeatSlot],
        decks: Option<SeatDecks>,
    ) -> Result<GameHandle, CreateError> {
        let names: Vec<String> = slots.iter().map(|slot| slot.name.clone()).collect();
        let (Some(records), Some(recipe)) = (&self.shared.records, recipe) else {
            return Ok(spawn_named(
                id,
                setup,
                MemoryStore::new(),
                self.shared.game,
                None,
                names,
                decks,
            )?);
        };
        let rows: Vec<NewSeat> = slots
            .iter()
            .map(|slot| NewSeat {
                name: slot.name.clone(),
                digest: slot.digest.as_ref().map(|digest| *digest.as_bytes()),
                state: slot.state,
                user: slot.user,
            })
            .collect();
        records.insert(id, recipe, meta, &rows).await?;
        let recorder = Recorder::new(records.clone(), id, vec![false; slots.len()]);
        let spawned = spawn_named(
            id,
            setup,
            records.store(),
            self.shared.game,
            Some(recorder),
            names,
            decks,
        );
        match spawned {
            Ok(handle) => Ok(handle),
            Err(error) => {
                // Not reachable after `validate`, but a record must not outlive its game.
                let reason = Cause::Unknown.reason();
                let _ = records.mark_failed(id, &error.to_string(), &reason).await;
                Err(error.into())
            }
        }
    }

    /// The game whose join code `typed` is, or `None`; a code that finds nothing and a text that
    /// cannot be a code are the same answer. Misses are counted per client `address` at time `now`
    /// (`AttemptLimits::code_misses_per_ip` in `code_window`); an address over the limit is turned
    /// away with [`Blocked`], even for a right code, until the window passes. Games are compared
    /// by digest without stopping at the first match.
    pub fn find_by_code(
        &self,
        typed: &str,
        address: IpAddr,
        now: Instant,
    ) -> Result<Option<GameId>, Blocked> {
        let attempt = self.shared.throttle.begin_code(now, address)?;
        let found = JoinCode::parse(typed).and_then(|code| {
            let presented = code.digest();
            let games = self.games();
            let mut found = None;
            for (id, entry) in games.iter() {
                if entry
                    .code
                    .as_ref()
                    .is_some_and(|stored| stored.matches(&presented))
                {
                    found = Some(*id);
                }
            }
            found
        });
        if found.is_some() {
            self.shared.throttle.succeeded(attempt);
        } else if attempt.exhausted() {
            tracing::warn!("a client has used up its join code misses; more are turned away");
        }
        Ok(found)
    }

    /// The actor of a game, for tests and admin views.
    pub fn handle(&self, id: GameId) -> Option<GameHandle> {
        self.games().get(&id).map(|entry| entry.handle.clone())
    }

    /// Who may find and join a live game; `None` if there is no such game.
    pub fn visibility(&self, id: GameId) -> Option<Visibility> {
        self.games().get(&id).map(|entry| entry.visibility)
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
        for (seat, slot) in entry.seats.iter().enumerate() {
            if slot
                .digest
                .as_ref()
                .is_some_and(|digest| digest.matches(&presented))
            {
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

    /// Maps an account to its seat in a live game; `None` for an unknown game and for an account
    /// that holds no seat there alike. The caller has already checked who the account is.
    pub fn authenticate_user(&self, id: GameId, user: UserId) -> Option<SeatAccess> {
        let games = self.games();
        let entry = games.get(&id)?;
        let seat = entry
            .seats
            .iter()
            .position(|slot| slot.user == Some(user))?;
        Some(SeatAccess {
            game: id,
            handle: entry.handle.clone(),
            seat: PlayerId::from_index(seat),
            removed: entry.alive.subscribe(),
        })
    }

    /// Binds `seat` of a live game to `user` if the seat is nobody's and the user holds no other
    /// seat of the game; a bound seat never changes hands. The database decides and memory
    /// follows, so racing accounts get one winner.
    pub async fn bind_seat(
        &self,
        game: GameId,
        seat: PlayerId,
        user: UserId,
    ) -> Result<Binding, RecordError> {
        let Some(records) = &self.shared.records else {
            return Ok(Binding::Missing);
        };
        let Some(turn) = self.games().get(&game).map(|entry| Arc::clone(&entry.turn)) else {
            return Ok(Binding::Missing);
        };
        // Seats do not change hands while an account is being bound.
        let _turn = turn.lock().await;
        {
            let games = self.games();
            let Some(entry) = games.get(&game) else {
                return Ok(Binding::Missing);
            };
            match entry.seats.get(seat.index()) {
                // An open seat has no token to bring anybody to it.
                None => return Ok(Binding::Missing),
                Some(slot) if slot.state == SeatState::Open => return Ok(Binding::Missing),
                Some(SeatSlot {
                    user: Some(holder), ..
                }) => {
                    return Ok(if *holder == user {
                        Binding::Already
                    } else {
                        Binding::Taken
                    });
                }
                Some(_) if entry.seats.iter().any(|slot| slot.user == Some(user)) => {
                    return Ok(Binding::Elsewhere);
                }
                Some(_) => {}
            }
        }
        let outcome = records.bind_user(game, seat, user).await?;
        if outcome == Binding::Bound
            && let Some(entry) = self.games().get_mut(&game)
            && let Some(slot) = entry.seats.get_mut(seat.index())
        {
            slot.user = Some(user);
        }
        Ok(outcome)
    }

    /// The games `user` has a seat in, newest first, at most [`MAX_LISTED_GAMES`]: those in their
    /// lobby, running and finished, whether or not they are still in memory. Empty for a
    /// registry without a database.
    pub async fn games_of(&self, user: UserId) -> Result<Vec<UserGame>, RecordError> {
        match &self.shared.records {
            Some(records) => records.list_for_user(user, MAX_LISTED_GAMES).await,
            None => Ok(Vec::new()),
        }
    }

    /// Takes a game that failed to resume off `user`'s list; the other players keep it. False when
    /// the user holds no seat in such a game or removed it already, and for a registry without a
    /// database, which holds no list.
    pub async fn remove_failed(&self, user: UserId, game: GameId) -> Result<bool, RecordError> {
        match &self.shared.records {
            Some(records) => records.remove_failed(game, user).await,
            None => Ok(false),
        }
    }

    /// The public facts of a game; `None` if there is no such game or its actor is gone.
    pub async fn describe(&self, id: GameId) -> Option<PublicGame> {
        let (handle, visibility, seats) = {
            let games = self.games();
            let entry = games.get(&id)?;
            let seats: Vec<SeatSummary> = entry
                .seats
                .iter()
                .enumerate()
                .map(|(seat, slot)| SeatSummary {
                    seat: PlayerId::from_index(seat),
                    name: slot.name.clone(),
                    state: slot.state,
                })
                .collect();
            (entry.handle.clone(), entry.visibility, seats)
        };
        let status = handle.status().await.ok()?;
        Some(PublicGame {
            visibility,
            started: status.started,
            seats,
            connected: status.connected,
            ready: status.ready,
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
