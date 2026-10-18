//! Boot: bringing the games an earlier process left unfinished back to life.

use std::time::Instant;

use mtg_engine::GameSetup;

use super::{Entry, Registry};
use crate::actor::resume_named;
use crate::game_id::GameId;
use crate::recipe::{Catalog, GameRecipe, RecipeError};
use crate::recorder::Recorder;
use crate::records::{GameRecord, GameRecords, RecordError, RecordStatus};
use crate::secret::TokenDigest;
use crate::user_id::UserId;

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
    names: Vec<String>,
    digests: Vec<TokenDigest>,
    users: Vec<Option<UserId>>,
    /// Whether the decks were compared with fingerprints stored in the recipe; false for a recipe
    /// that predates them.
    checked: bool,
}

/// Why a stored game cannot be loaded, and the seat the reason is about when it is about one.
struct Unloadable {
    reason: String,
    seat: Option<usize>,
}

impl Unloadable {
    fn new(reason: String) -> Self {
        Unloadable { reason, seat: None }
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
            names,
            digests,
            users,
            checked,
        } = match unpacked {
            Ok(unpacked) => unpacked,
            Err(Unloadable { reason, seat }) => return give_up(records, id, seat, &reason).await,
        };
        let joined = record.seats.iter().map(|seat| seat.joined).collect();
        let recorder = Recorder::new(records.clone(), id, joined);
        // Replaying a log is where a bug would panic; that must cost one game, not the boot.
        let resuming = tokio::spawn(resume_named(
            records.store(),
            id,
            setup,
            self.shared.game,
            Some(recorder),
            names.clone(),
        ));
        let handle = match resuming.await {
            Ok(Ok(handle)) => handle,
            Ok(Err(error)) => return give_up(records, id, None, &error.to_string()).await,
            Err(error) => {
                let reason = format!("resuming failed: {error}");
                return give_up(records, id, None, &reason).await;
            }
        };
        let Ok(status) = handle.status().await else {
            return give_up(records, id, None, "the game's task stopped while resuming").await;
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
        if status.started && record.status == RecordStatus::Lobby {
            note(records.mark_running(id).await, id);
        }
        // A resumed game keeps the age it had, so the lobby clock does not restart with the boot.
        let now = Instant::now();
        let created = now.checked_sub(record.age).unwrap_or(now);
        self.games()
            .insert(id, Entry::new(handle, names, digests, users, created));
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
                Unloadable::new(format!("the cards of its decks cannot be had: {error}"))
            }),
            (None, false) => Err(Unloadable::new(
                "its decks name cards by oracle id and this server has no card database".to_owned(),
            )),
        }
    }
}

/// Decodes the stored recipe.
fn decode(record: &GameRecord) -> Result<GameRecipe, Unloadable> {
    serde_json::from_value(record.recipe.clone())
        .map_err(|error| Unloadable::new(format!("the recipe does not decode: {error}")))
}

/// Rebuilds the setup (which refuses a deck that changed since the game was created) and checks the
/// seat rows against it.
fn unpack(
    record: &GameRecord,
    recipe: &GameRecipe,
    catalog: &Catalog,
) -> Result<Unpacked, Unloadable> {
    let setup = recipe.build_with(catalog).map_err(|error| match error {
        // The reason leads with what happened to the game, and the log names the seat.
        RecipeError::DeckChanged { seat, .. } => Unloadable {
            reason: error.to_string(),
            seat: Some(seat),
        },
        _ => Unloadable::new(format!("the recipe does not build: {error}")),
    })?;
    let seats = setup.players.len();
    let numbered = record
        .seats
        .iter()
        .enumerate()
        .all(|(index, seat)| seat.seat == index);
    if record.seats.len() != seats || !numbered {
        let found: Vec<usize> = record.seats.iter().map(|seat| seat.seat).collect();
        return Err(Unloadable::new(format!(
            "the recipe has {seats} seats but the records hold seats {found:?}"
        )));
    }
    Ok(Unpacked {
        checked: recipe.is_fingerprinted(),
        names: record.seats.iter().map(|seat| seat.name.clone()).collect(),
        digests: record
            .seats
            .iter()
            .map(|seat| TokenDigest::from_bytes(seat.token_digest))
            .collect(),
        users: record.seats.iter().map(|seat| seat.user).collect(),
        setup,
    })
}

/// Logs why a game cannot come back, with the seat when the reason is about one, and marks it
/// failed so the next boot skips it.
async fn give_up(records: &GameRecords, id: GameId, seat: Option<usize>, reason: &str) -> Outcome {
    tracing::error!(game = %id, seat, %reason, "game cannot be resumed; marked failed");
    note(records.mark_failed(id, reason).await, id);
    Outcome::Failed
}

/// Logs a record update that failed; boot goes on, because the next one repeats the repair.
fn note(result: Result<(), RecordError>, id: GameId) {
    if let Err(error) = result {
        tracing::warn!(game = %id, %error, "could not update a game record while resuming");
    }
}
