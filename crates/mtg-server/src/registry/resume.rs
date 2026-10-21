//! Boot: bringing the games an earlier process left unfinished back to life.

use std::time::Instant;

use mtg_engine::GameSetup;

use super::{Cause, Entry, Parts, Registry, SeatSlot};
use crate::actor::resume_named;
use crate::game_id::GameId;
use crate::recipe::{Catalog, FormatKind, GameRecipe, RecipeError};
use crate::recorder::Recorder;
use crate::records::{GameRecord, GameRecords, RecordError, RecordStatus, SeatState};
use crate::seat_deck::SeatDecks;
use crate::secret::TokenDigest;

/// What [`Registry::resume`] did with the games it found.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ResumeReport {
    /// Games running again, lobbies included.
    pub resumed: usize,
    /// Of those, the games that are back in their lobby (nothing was stored of them).
    pub lobbies: usize,
    /// Games whose log shows they ended; their record was corrected and they were not loaded.
    pub finished: usize,
    /// Lobbies older than the retention rule, dropped instead of resumed.
    pub expired: usize,
    /// Games that could not be loaded; each is marked failed in its record.
    pub failed: Vec<GameId>,
}

/// What became of one stored game.
enum Outcome {
    Live { lobby: bool },
    Finished,
    Expired,
    Failed,
}

/// A recipe turned back into what the registry needs.
struct Unpacked {
    setup: GameSetup,
    format: FormatKind,
    seats: Vec<SeatSlot>,
    /// Whether the decks were compared with fingerprints stored in the recipe; false for a recipe
    /// that predates them.
    checked: bool,
    /// The decks of the seats if the game turns out to be in its lobby.
    decks: SeatDecks,
}

/// Why a stored game cannot be loaded: the `detail` for the log and the record's `failure`, the
/// `cause` its players read, and the seat it is about when it is about one.
struct Unloadable {
    detail: String,
    cause: Cause,
    seat: Option<usize>,
}

impl Unloadable {
    fn new(detail: String, cause: Cause) -> Self {
        Unloadable {
            detail,
            cause,
            seat: None,
        }
    }
}

impl Registry {
    /// Loads every game that is still in its lobby or running and starts its actor from its log,
    /// keeping the seat tokens valid, so a client that comes back with its old token lands in the
    /// same game.
    ///
    /// A lobby has no log: it returns to its lobby (everybody must send `Ready` again) and keeps
    /// its age, so a lobby older than the retention rule is dropped instead. A game whose recipe or
    /// log does not load is marked failed and skipped, and so is one whose deck no longer matches
    /// the fingerprint stored at its creation (no event is replayed under other cards); a recipe
    /// that predates the fingerprints resumes unchecked, with a warning. Only an unreachable
    /// database is an error.
    /// Games already resumed count against the cap, which limits creation only. Without a database
    /// this does nothing.
    pub async fn resume(&self) -> Result<ResumeReport, RecordError> {
        let Some(records) = self.shared.records.clone() else {
            return Ok(ResumeReport::default());
        };
        let mut report = ResumeReport::default();
        for record in records.unfinished().await? {
            let id = record.id;
            match self.revive(&records, record).await {
                Outcome::Live { lobby } => {
                    report.resumed += 1;
                    report.lobbies += usize::from(lobby);
                }
                Outcome::Finished => report.finished += 1,
                Outcome::Expired => report.expired += 1,
                Outcome::Failed => report.failed.push(id),
            }
        }
        Ok(report)
    }

    async fn revive(&self, records: &GameRecords, record: GameRecord) -> Outcome {
        let id = record.id;
        let unpacked = match decode(&record) {
            Ok(recipe) => match self.catalog_of(&recipe).await {
                Ok(catalog) => unpack(&record, &recipe, &catalog),
                Err(unloadable) => Err(unloadable),
            },
            Err(unloadable) => Err(unloadable),
        };
        let Unpacked {
            setup,
            format,
            seats,
            checked,
            decks,
        } = match unpacked {
            Ok(unpacked) => unpacked,
            Err(unloadable) => return give_up(records, id, unloadable).await,
        };
        let house_rules = setup.house_rules;
        let names: Vec<String> = seats.iter().map(|slot| slot.name.clone()).collect();
        let joined = record.seats.iter().map(|seat| seat.joined).collect();
        let recorder = Recorder::new(records.clone(), id, joined);
        // Replaying a log is where a bug would panic; that must cost one game, not the boot.
        let resuming = tokio::spawn(resume_named(
            records.store(),
            id,
            setup,
            self.shared.game,
            Some(recorder),
            names,
            Some(decks),
        ));
        let handle = match resuming.await {
            Ok(Ok(handle)) => handle,
            Ok(Err(error)) => {
                return give_up(records, id, replay_failed(error.to_string())).await;
            }
            Err(error) => {
                return give_up(
                    records,
                    id,
                    replay_failed(format!("resuming failed: {error}")),
                )
                .await;
            }
        };
        let Ok(status) = handle.status().await else {
            let detail = "the game's task stopped while resuming".to_owned();
            return give_up(records, id, replay_failed(detail)).await;
        };

        if let Some(result) = status.result {
            // The game ended but its record never heard; dropping the handle stops the actor.
            note(records.mark_finished(id, result).await, id);
            return Outcome::Finished;
        }
        if !status.started && record.age >= self.shared.retention.lobby_after {
            note(records.mark_expired(id).await, id);
            return Outcome::Expired;
        }
        // A game cannot begin while a seat is open, because nobody there can press Ready.
        if status.started && seats.iter().any(|slot| slot.state == SeatState::Open) {
            let detail = "the game began with a seat nobody holds".to_owned();
            return give_up(records, id, Unloadable::new(detail, Cause::EmptySeat)).await;
        }
        if status.started && record.status == RecordStatus::Lobby {
            note(records.mark_running(id).await, id);
        }
        // A resumed game keeps the age it had, so the lobby clock does not restart with the boot.
        let now = Instant::now();
        let created = now.checked_sub(record.age).unwrap_or(now);
        let parts = Parts {
            visibility: record.visibility,
            format,
            house_rules,
            code: record.code_digest.map(TokenDigest::from_bytes),
            recorded: true,
            seats,
        };
        self.games().insert(id, Entry::new(handle, parts, created));
        if checked {
            tracing::info!(game = %id, started = status.started, events = status.events, "game resumed");
        } else {
            tracing::warn!(
                game = %id,
                started = status.started,
                events = status.events,
                "game resumed without a deck check: its recipe has no deck fingerprints (stored before they existed)"
            );
        }
        Outcome::Live {
            lobby: !status.started,
        }
    }
}

impl Registry {
    /// The definitions of the cards a recipe names by oracle id, assembled from the database as it
    /// is now; none (and no database access) for a recipe that names none.
    async fn catalog_of(&self, recipe: &GameRecipe) -> Result<Catalog, Unloadable> {
        let ids = recipe.oracle_ids();
        match (&self.shared.cards, ids.is_empty()) {
            (_, true) => Ok(Catalog::default()),
            (Some(cards), false) => cards.catalog_for(recipe).await.map_err(|error| {
                let detail = format!("the cards of its decks cannot be had: {error}");
                Unloadable::new(detail, Cause::Cards)
            }),
            (None, false) => Err(Unloadable::new(
                "its decks name cards by oracle id and this server has no card database".to_owned(),
                Cause::Cards,
            )),
        }
    }
}

/// The log of a game cannot be loaded or replayed.
fn replay_failed(detail: String) -> Unloadable {
    Unloadable::new(detail, Cause::Replay)
}

/// Decodes the stored recipe.
fn decode(record: &GameRecord) -> Result<GameRecipe, Unloadable> {
    serde_json::from_value(record.recipe.clone()).map_err(|error| {
        let detail = format!("the recipe does not decode: {error}");
        Unloadable::new(detail, Cause::Setup)
    })
}

/// Rebuilds the setup (which refuses a deck that changed since the game was created) and checks the
/// seat rows against it.
fn unpack(
    record: &GameRecord,
    recipe: &GameRecipe,
    catalog: &Catalog,
) -> Result<Unpacked, Unloadable> {
    let setup = recipe.build_with(catalog).map_err(|error| match error {
        // The detail leads with what happened to the game, and the log names the seat.
        RecipeError::DeckChanged { seat, .. } => Unloadable {
            detail: error.to_string(),
            cause: Cause::DeckChanged {
                player: player_of(record, seat),
            },
            seat: Some(seat),
        },
        RecipeError::UnknownDeck { .. } | RecipeError::UnknownCards { .. } => {
            Unloadable::new(format!("the recipe does not build: {error}"), Cause::Cards)
        }
        _ => Unloadable::new(format!("the recipe does not build: {error}"), Cause::Setup),
    })?;
    let seats = setup.players.len();
    let numbered = record
        .seats
        .iter()
        .enumerate()
        .all(|(index, seat)| seat.seat == index);
    if record.seats.len() != seats || !numbered {
        let found: Vec<usize> = record.seats.iter().map(|seat| seat.seat).collect();
        return Err(Unloadable::new(
            format!("the recipe has {seats} seats but the records hold seats {found:?}"),
            Cause::Seats,
        ));
    }
    Ok(Unpacked {
        checked: recipe.is_fingerprinted(),
        decks: SeatDecks::of_recipe(recipe),
        format: recipe.format,
        seats: record
            .seats
            .iter()
            .map(|seat| SeatSlot {
                state: seat.state,
                name: seat.name.clone(),
                digest: seat.token_digest.map(TokenDigest::from_bytes),
                user: seat.user,
            })
            .collect(),
        setup,
    })
}

/// Who plays `seat`, as the players know them: the seat's name, else its place at the table.
fn player_of(record: &GameRecord, seat: usize) -> String {
    match record.seats.iter().find(|row| row.seat == seat) {
        Some(row) if !row.name.is_empty() => row.name.clone(),
        _ => format!("seat {}", seat + 1),
    }
}

/// Logs why a game cannot come back, with the seat when the reason is about one, and marks it
/// failed so the next boot skips it. The record keeps the detail for the operator and the cause in
/// words for the players.
async fn give_up(records: &GameRecords, id: GameId, unloadable: Unloadable) -> Outcome {
    let Unloadable {
        detail,
        cause,
        seat,
    } = unloadable;
    tracing::error!(game = %id, seat, reason = %detail, "game cannot be resumed; marked failed");
    note(records.mark_failed(id, &detail, &cause.reason()).await, id);
    Outcome::Failed
}

/// Logs a record update that failed; boot goes on, because the next one repeats the repair.
fn note(result: Result<(), RecordError>, id: GameId) {
    if let Err(error) = result {
        tracing::warn!(game = %id, %error, "could not update a game record while resuming");
    }
}
