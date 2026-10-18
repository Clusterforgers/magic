//! Game and seat records in Postgres: what a restart needs to bring a game back.
//!
//! `games` holds the recipe and the game's status, `seats` the display name, the SHA-256 of the
//! token, whether the seat ever joined and the account it belongs to, if any. The event log lives
//! apart in `game_events`. Run [`migrate`](crate::migrate) before use.

mod users;

use std::collections::HashMap;
use std::time::Duration;

use mtg_core::ids::PlayerId;
use mtg_engine::state::GameResult;
use sqlx::PgPool;
use thiserror::Error;
use uuid::Uuid;

use crate::game_id::GameId;
use crate::pg_store::PgEventStore;
use crate::recipe::GameRecipe;
use crate::user_id::UserId;

pub use users::{Binding, UserGame, UserGameSeat};

/// Why the records could not be read or written.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum RecordError {
    /// The database could not be reached or refused the statement.
    #[error("game records unavailable: {0}")]
    Unavailable(String),
    /// A value could not be encoded for storage.
    #[error("game record does not encode: {0}")]
    Encode(String),
    /// A stored row holds a value this server does not know.
    #[error("game record is unusable: {0}")]
    Corrupt(String),
}

impl From<sqlx::Error> for RecordError {
    fn from(error: sqlx::Error) -> Self {
        RecordError::Unavailable(error.to_string())
    }
}

/// Where a game is in its life.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecordStatus {
    /// Created, not every seat has said it is ready.
    Lobby,
    /// The engine started and the game has no result yet.
    Running,
    /// The game ended with a result.
    Finished,
    /// Nobody came back in time, so the server dropped it.
    Expired,
    /// The recipe, a deck that changed since the game was created, or the log could not be loaded on boot.
    Failed,
}

impl RecordStatus {
    /// The text stored in `games.status`.
    pub const fn as_str(self) -> &'static str {
        match self {
            RecordStatus::Lobby => "lobby",
            RecordStatus::Running => "running",
            RecordStatus::Finished => "finished",
            RecordStatus::Expired => "expired",
            RecordStatus::Failed => "failed",
        }
    }

    fn parse(text: &str) -> Result<RecordStatus, RecordError> {
        [
            RecordStatus::Lobby,
            RecordStatus::Running,
            RecordStatus::Finished,
            RecordStatus::Expired,
            RecordStatus::Failed,
        ]
        .into_iter()
        .find(|status| status.as_str() == text)
        .ok_or_else(|| RecordError::Corrupt(format!("unknown game status {text:?}")))
    }
}

/// A seat to store with a new game.
#[derive(Debug, Clone)]
pub(crate) struct NewSeat {
    pub(crate) name: String,
    pub(crate) digest: [u8; 32],
    /// The account the seat belongs to from the start.
    pub(crate) user: Option<UserId>,
}

/// A stored seat.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SeatRecord {
    /// The seat number.
    pub seat: usize,
    /// Its display name.
    pub name: String,
    /// SHA-256 of its token.
    pub token_digest: [u8; 32],
    /// Whether anybody ever connected to the seat.
    pub joined: bool,
    /// The account the seat is bound to; guests' seats have none.
    pub user: Option<UserId>,
}

/// A stored game with its seats in seat order.
#[derive(Debug, Clone, PartialEq)]
pub struct GameRecord {
    /// The game.
    pub id: GameId,
    /// Where it is in its life.
    pub status: RecordStatus,
    /// The recipe as stored; decoding it is the caller's business, since it may be damaged.
    pub recipe: serde_json::Value,
    /// How the game ended, once it did.
    pub result: Option<GameResult>,
    /// Why the game was marked failed.
    pub failure: Option<String>,
    /// Time since the record was created, by the database's clock.
    pub age: Duration,
    /// The seats that could be read.
    pub seats: Vec<SeatRecord>,
}

/// The `games` and `seats` tables; clones share the pool.
#[derive(Debug, Clone)]
pub struct GameRecords {
    pool: PgPool,
}

/// A `games` row before its seats are attached.
struct GameRow {
    id: Uuid,
    status: String,
    recipe: serde_json::Value,
    result: Option<serde_json::Value>,
    failure: Option<String>,
    age_secs: f64,
}

impl GameRow {
    fn into_record(self, seats: Vec<SeatRecord>) -> Result<GameRecord, RecordError> {
        Ok(GameRecord {
            id: GameId(self.id),
            status: RecordStatus::parse(&self.status)?,
            recipe: self.recipe,
            // A result this server cannot read only matters for display, never for resuming.
            result: self.result.and_then(|v| serde_json::from_value(v).ok()),
            failure: self.failure,
            age: age(self.age_secs),
            seats,
        })
    }
}

/// The longest failure reason kept, in characters.
const MAX_FAILURE_CHARS: usize = 500;

impl GameRecords {
    /// Records over `pool`.
    pub fn new(pool: PgPool) -> Self {
        GameRecords { pool }
    }

    /// The event log store over the same pool.
    pub(crate) fn store(&self) -> PgEventStore {
        PgEventStore::new(self.pool.clone())
    }

    /// Stores a new game in its lobby together with its seats, or nothing at all.
    pub(crate) async fn insert(
        &self,
        id: GameId,
        recipe: &GameRecipe,
        seats: &[NewSeat],
    ) -> Result<(), RecordError> {
        let recipe =
            serde_json::to_value(recipe).map_err(|e| RecordError::Encode(e.to_string()))?;
        let numbers: Vec<i32> = (0..seats.len())
            .map(|seat| i32::try_from(seat).unwrap_or(i32::MAX))
            .collect();
        let names: Vec<String> = seats.iter().map(|seat| seat.name.clone()).collect();
        let digests: Vec<Vec<u8>> = seats.iter().map(|seat| seat.digest.to_vec()).collect();
        let users: Vec<Option<Uuid>> = seats
            .iter()
            .map(|seat| seat.user.map(UserId::uuid))
            .collect();

        let mut tx = self.pool.begin().await?;
        sqlx::query!(
            "INSERT INTO games (id, status, recipe) VALUES ($1, 'lobby', $2)",
            id.uuid(),
            recipe,
        )
        .execute(&mut *tx)
        .await?;
        sqlx::query!(
            "INSERT INTO seats (game_id, seat, name, token_digest, user_id)
             SELECT $1, t.seat, t.name, t.digest, t.user_id
             FROM unnest($2::int4[], $3::text[], $4::bytea[], $5::uuid[])
                  AS t(seat, name, digest, user_id)",
            id.uuid(),
            &numbers[..],
            &names[..],
            &digests[..],
            // Seats of guests have no account: the macro cannot see that elements may be NULL.
            &users[..] as _,
        )
        .execute(&mut *tx)
        .await?;
        Ok(tx.commit().await?)
    }

    /// One game by id, whatever its status.
    pub async fn get(&self, id: GameId) -> Result<Option<GameRecord>, RecordError> {
        let row = sqlx::query_as!(
            GameRow,
            r#"SELECT id, status, recipe, result, failure,
                      extract(epoch FROM now() - created_at)::float8 AS "age_secs!"
               FROM games WHERE id = $1"#,
            id.uuid(),
        )
        .fetch_optional(&self.pool)
        .await?;
        let Some(row) = row else {
            return Ok(None);
        };
        let mut seats = self.seats_of(&[row.id]).await?;
        let seats = seats.remove(&row.id).unwrap_or_default();
        row.into_record(seats).map(Some)
    }

    /// Every game still in its lobby or running, oldest first.
    pub async fn unfinished(&self) -> Result<Vec<GameRecord>, RecordError> {
        let rows = sqlx::query_as!(
            GameRow,
            r#"SELECT id, status, recipe, result, failure,
                      extract(epoch FROM now() - created_at)::float8 AS "age_secs!"
               FROM games WHERE status IN ('lobby', 'running') ORDER BY created_at, id"#,
        )
        .fetch_all(&self.pool)
        .await?;
        let ids: Vec<Uuid> = rows.iter().map(|row| row.id).collect();
        let mut seats = self.seats_of(&ids).await?;
        rows.into_iter()
            .map(|row| {
                let seats = seats.remove(&row.id).unwrap_or_default();
                row.into_record(seats)
            })
            .collect()
    }

    /// The readable seats of `games`, in seat order. A damaged row is left out, which the caller
    /// sees as a seat count that does not match the recipe.
    async fn seats_of(
        &self,
        games: &[Uuid],
    ) -> Result<HashMap<Uuid, Vec<SeatRecord>>, RecordError> {
        let rows = sqlx::query!(
            "SELECT game_id, seat, name, token_digest, joined, user_id
             FROM seats WHERE game_id = ANY($1) ORDER BY game_id, seat",
            games,
        )
        .fetch_all(&self.pool)
        .await?;
        let mut by_game: HashMap<Uuid, Vec<SeatRecord>> = HashMap::new();
        for row in rows {
            let (Ok(seat), Ok(token_digest)) = (
                usize::try_from(row.seat),
                <[u8; 32]>::try_from(row.token_digest.as_slice()),
            ) else {
                continue;
            };
            by_game.entry(row.game_id).or_default().push(SeatRecord {
                seat,
                name: row.name,
                token_digest,
                joined: row.joined,
                user: row.user_id.map(UserId),
            });
        }
        Ok(by_game)
    }

    /// Notes that somebody connected to `seat`.
    pub(crate) async fn mark_joined(&self, id: GameId, seat: PlayerId) -> Result<(), RecordError> {
        let seat = i32::try_from(seat.index()).unwrap_or(i32::MAX);
        sqlx::query!(
            "UPDATE seats SET joined = true WHERE game_id = $1 AND seat = $2 AND NOT joined",
            id.uuid(),
            seat,
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Notes that the lobby is over and the engine runs.
    pub(crate) async fn mark_running(&self, id: GameId) -> Result<(), RecordError> {
        sqlx::query!(
            "UPDATE games SET status = 'running', started_at = now()
             WHERE id = $1 AND status = 'lobby'",
            id.uuid(),
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Notes the result; a game already finished, expired or failed keeps its status.
    pub(crate) async fn mark_finished(
        &self,
        id: GameId,
        result: GameResult,
    ) -> Result<(), RecordError> {
        let result =
            serde_json::to_value(result).map_err(|e| RecordError::Encode(e.to_string()))?;
        sqlx::query!(
            "UPDATE games SET status = 'finished', result = $2, finished_at = now()
             WHERE id = $1 AND status IN ('lobby', 'running')",
            id.uuid(),
            result,
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Notes that the server dropped an unfinished game for good.
    pub(crate) async fn mark_expired(&self, id: GameId) -> Result<(), RecordError> {
        sqlx::query!(
            "UPDATE games SET status = 'expired' WHERE id = $1 AND status IN ('lobby', 'running')",
            id.uuid(),
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Notes that the game could not be loaded and why.
    pub(crate) async fn mark_failed(&self, id: GameId, reason: &str) -> Result<(), RecordError> {
        let reason: String = reason.chars().take(MAX_FAILURE_CHARS).collect();
        sqlx::query!(
            "UPDATE games SET status = 'failed', failure = $2
             WHERE id = $1 AND status IN ('lobby', 'running')",
            id.uuid(),
            reason,
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }
}

/// A database age in seconds as a duration; a clock that ran backwards counts as zero.
fn age(seconds: f64) -> Duration {
    Duration::try_from_secs_f64(seconds).unwrap_or(Duration::ZERO)
}

#[cfg(test)]
mod tests {
    //! Goal: every status has one stored spelling that parses back, unknown spellings are refused, and a database age never panics.
    use super::*;
    use rstest::rstest;

    #[rstest]
    #[case(RecordStatus::Lobby, "lobby")]
    #[case(RecordStatus::Running, "running")]
    #[case(RecordStatus::Finished, "finished")]
    #[case(RecordStatus::Expired, "expired")]
    #[case(RecordStatus::Failed, "failed")]
    fn statuses_round_trip(#[case] status: RecordStatus, #[case] text: &str) {
        assert_eq!(status.as_str(), text);
        assert_eq!(RecordStatus::parse(text), Ok(status));
    }

    #[rstest]
    #[case("")]
    #[case("Lobby")]
    #[case("done")]
    fn unknown_statuses_are_corrupt(#[case] text: &str) {
        assert!(matches!(
            RecordStatus::parse(text),
            Err(RecordError::Corrupt(_))
        ));
    }

    #[rstest]
    #[case(0.0, Duration::ZERO)]
    #[case(1.5, Duration::from_millis(1500))]
    #[case(-3.0, Duration::ZERO)]
    #[case(f64::NAN, Duration::ZERO)]
    #[case(f64::INFINITY, Duration::ZERO)]
    fn ages_are_forgiving(#[case] seconds: f64, #[case] expected: Duration) {
        assert_eq!(age(seconds), expected);
    }
}
