//! Boot: bringing the games an earlier process left unfinished back to life.

use std::time::Instant;

use mtg_engine::GameSetup;

use super::{Entry, Registry};
use crate::actor::resume_named;
use crate::game_id::GameId;
use crate::recipe::GameRecipe;
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
}

impl Registry {
    /// Loads every game that is still in its lobby or running and starts its actor from its log,
    /// keeping the seat tokens valid, so a client that comes back with its old token lands in the
    /// same game.
    ///
    /// A lobby has no log: it returns to its lobby (everybody must send `Ready` again) and keeps
    /// its age, so a lobby older than the retention rule is dropped instead. A game whose recipe or
    /// log does not load is marked failed and skipped. Only an unreachable database is an error.
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
        let Unpacked {
            setup,
            names,
            digests,
            users,
        } = match unpack(&record) {
            Ok(unpacked) => unpacked,
            Err(reason) => return give_up(records, id, &reason).await,
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
            Ok(Err(error)) => return give_up(records, id, &error.to_string()).await,
            Err(error) => return give_up(records, id, &format!("resuming failed: {error}")).await,
        };
        let Ok(status) = handle.status().await else {
            return give_up(records, id, "the game's task stopped while resuming").await;
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
        tracing::info!(game = %id, started = status.started, events = status.events, "game resumed");
        Outcome::Live {
            lobby: !status.started,
        }
    }
}

/// Decodes the recipe and checks the seat rows against it.
fn unpack(record: &GameRecord) -> Result<Unpacked, String> {
    let recipe: GameRecipe = serde_json::from_value(record.recipe.clone())
        .map_err(|error| format!("the recipe does not decode: {error}"))?;
    let setup = recipe
        .build()
        .map_err(|error| format!("the recipe does not build: {error}"))?;
    let seats = setup.players.len();
    let numbered = record
        .seats
        .iter()
        .enumerate()
        .all(|(index, seat)| seat.seat == index);
    if record.seats.len() != seats || !numbered {
        let found: Vec<usize> = record.seats.iter().map(|seat| seat.seat).collect();
        return Err(format!(
            "the recipe has {seats} seats but the records hold seats {found:?}"
        ));
    }
    Ok(Unpacked {
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

/// Logs why a game cannot come back and marks it failed so the next boot skips it.
async fn give_up(records: &GameRecords, id: GameId, reason: &str) -> Outcome {
    tracing::error!(game = %id, %reason, "game cannot be resumed; marked failed");
    note(records.mark_failed(id, reason).await, id);
    Outcome::Failed
}

/// Logs a record update that failed; boot goes on, because the next one repeats the repair.
fn note(result: Result<(), RecordError>, id: GameId) {
    if let Err(error) = result {
        tracing::warn!(game = %id, %error, "could not update a game record while resuming");
    }
}
