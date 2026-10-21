//! Game and seat records in Postgres: what a restart needs to bring a game back.
//!
//! `games` holds the recipe, the game's status, who may find it ([`Visibility`]) and the SHA-256 of
//! its join code. `seats` holds each seat's display name, how it is held ([`SeatState`]), the
//! SHA-256 of its token (none while it is open), whether the seat ever joined and the account it
//! belongs to, if any. The event log lives apart in `game_events`. Run [`migrate`](crate::migrate)
//! before use.

mod claims;
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

pub(crate) use claims::Claimed;
/// Who may find and join a game; the wire type, stored as its `as_str` text.
pub use mtg_protocol::rest::Visibility;
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
    /// The server dropped it because nobody came back in time, or its host closed the lobby.
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

/// The visibility stored in `games.visibility`; text no visibility is spelled as is corrupt.
fn parse_visibility(text: &str) -> Result<Visibility, RecordError> {
    Visibility::ALL
        .into_iter()
        .find(|visibility| visibility.as_str() == text)
        .ok_or_else(|| RecordError::Corrupt(format!("unknown game visibility {text:?}")))
}

/// How a seat is held.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SeatState {
    /// Its token was made with the game and handed out in the create response (the seat link).
    Issued,
    /// Set aside for a join: nobody holds it and it has no token.
    Open,
    /// A join took it and was given its token; it can be released until the game starts.
    Claimed,
}

impl SeatState {
    /// The text stored in `seats.state`.
    pub const fn as_str(self) -> &'static str {
        match self {
            SeatState::Issued => "issued",
            SeatState::Open => "open",
            SeatState::Claimed => "claimed",
        }
    }

    fn parse(text: &str) -> Result<SeatState, RecordError> {
        [SeatState::Issued, SeatState::Open, SeatState::Claimed]
            .into_iter()
            .find(|state| state.as_str() == text)
            .ok_or_else(|| RecordError::Corrupt(format!("unknown seat state {text:?}")))
    }
}

/// What a new game is stored with besides its recipe and seats.
#[derive(Debug, Clone, Copy)]
pub(crate) struct NewGame {
    pub(crate) visibility: Visibility,
    /// SHA-256 of the join code.
    pub(crate) code_digest: Option<[u8; 32]>,
}

/// A seat to store with a new game.
#[derive(Debug, Clone)]
pub(crate) struct NewSeat {
    pub(crate) name: String,
    /// SHA-256 of the token; none for an open seat.
    pub(crate) digest: Option<[u8; 32]>,
    pub(crate) state: SeatState,
    /// The account the seat belongs to from the start.
    pub(crate) user: Option<UserId>,
}

/// A stored seat.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SeatRecord {
    /// The seat number.
    pub seat: usize,
    /// Its display name; empty while the seat is open.
    pub name: String,
    /// SHA-256 of its token; none while the seat is open.
    pub token_digest: Option<[u8; 32]>,
    /// How the seat is held.
    pub state: SeatState,
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
    /// Who may find and join it.
    pub visibility: Visibility,
    /// SHA-256 of its join code; none for a game stored before codes existed.
    pub code_digest: Option<[u8; 32]>,
    /// The recipe as stored; decoding it is the caller's business, since it may be damaged.
    pub recipe: serde_json::Value,
    /// How the game ended, once it did.
    pub result: Option<GameResult>,
    /// Why the game was marked failed, for the operator: errors and fingerprints, never shown to players.
    pub failure: Option<String>,
    /// Why it failed in words for its players, free of internals; none for a game marked failed before
    /// the reason was kept.
    pub failure_reason: Option<String>,
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
    visibility: String,
    code_digest: Option<Vec<u8>>,
    recipe: serde_json::Value,
    result: Option<serde_json::Value>,
    failure: Option<String>,
    failure_reason: Option<String>,
    age_secs: f64,
}

impl GameRow {
    fn into_record(self, seats: Vec<SeatRecord>) -> Result<GameRecord, RecordError> {
        let code_digest = match self.code_digest {
            None => None,
            Some(bytes) => Some(<[u8; 32]>::try_from(bytes.as_slice()).map_err(|_| {
                RecordError::Corrupt("a join code digest is not 32 bytes".to_owned())
            })?),
        };
        Ok(GameRecord {
            id: GameId(self.id),
            status: RecordStatus::parse(&self.status)?,
            visibility: parse_visibility(&self.visibility)?,
            code_digest,
            recipe: self.recipe,
            // A result this server cannot read only matters for display, never for resuming.
            result: self.result.and_then(|v| serde_json::from_value(v).ok()),
            failure: self.failure,
            failure_reason: self.failure_reason,
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
        game: &NewGame,
        seats: &[NewSeat],
    ) -> Result<(), RecordError> {
        let recipe =
            serde_json::to_value(recipe).map_err(|e| RecordError::Encode(e.to_string()))?;
        let numbers: Vec<i32> = (0..seats.len())
            .map(|seat| i32::try_from(seat).unwrap_or(i32::MAX))
            .collect();
        let names: Vec<String> = seats.iter().map(|seat| seat.name.clone()).collect();
        let digests: Vec<Option<Vec<u8>>> = seats
            .iter()
            .map(|seat| seat.digest.map(|digest| digest.to_vec()))
            .collect();
        let states: Vec<String> = seats
            .iter()
            .map(|seat| seat.state.as_str().to_owned())
            .collect();
        let users: Vec<Option<Uuid>> = seats
            .iter()
            .map(|seat| seat.user.map(UserId::uuid))
            .collect();
        let code = game.code_digest.as_ref().map(<[u8; 32]>::as_slice);

        let mut tx = self.pool.begin().await?;
        sqlx::query!(
            "INSERT INTO games (id, status, recipe, visibility, code_digest)
             VALUES ($1, 'lobby', $2, $3, $4)",
            id.uuid(),
            recipe,
            game.visibility.as_str(),
            code,
        )
        .execute(&mut *tx)
        .await?;
        sqlx::query!(
            "INSERT INTO seats (game_id, seat, name, token_digest, state, user_id)
             SELECT $1, t.seat, t.name, t.digest, t.state, t.user_id
             FROM unnest($2::int4[], $3::text[], $4::bytea[], $5::text[], $6::uuid[])
                  AS t(seat, name, digest, state, user_id)",
            id.uuid(),
            &numbers[..],
            &names[..],
            // An open seat has no token and a guest's seat no account: the macro cannot see that
            // elements may be NULL.
            &digests[..] as _,
            &states[..],
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
            r#"SELECT id, status, visibility, code_digest, recipe, result, failure, failure_reason,
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
            r#"SELECT id, status, visibility, code_digest, recipe, result, failure, failure_reason,
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
            "SELECT game_id, seat, name, token_digest, state, joined, user_id
             FROM seats WHERE game_id = ANY($1) ORDER BY game_id, seat",
            games,
        )
        .fetch_all(&self.pool)
        .await?;
        let mut by_game: HashMap<Uuid, Vec<SeatRecord>> = HashMap::new();
        for row in rows {
            let token_digest = match row.token_digest.as_deref().map(<[u8; 32]>::try_from) {
                None => None,
                Some(Ok(digest)) => Some(digest),
                Some(Err(_)) => continue,
            };
            let (Ok(seat), Ok(state)) = (usize::try_from(row.seat), SeatState::parse(&row.state))
            else {
                continue;
            };
            by_game.entry(row.game_id).or_default().push(SeatRecord {
                seat,
                name: row.name,
                token_digest,
                state,
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

    /// Replaces the recipe of a game that is still in its lobby, which is how the decks its seats
    /// chose are kept. An error if the game is not in its lobby, since a game that began runs under
    /// the recipe it began with.
    pub(crate) async fn set_recipe(
        &self,
        id: GameId,
        recipe: &GameRecipe,
    ) -> Result<(), RecordError> {
        let recipe =
            serde_json::to_value(recipe).map_err(|e| RecordError::Encode(e.to_string()))?;
        let updated = sqlx::query!(
            "UPDATE games SET recipe = $2 WHERE id = $1 AND status = 'lobby'",
            id.uuid(),
            recipe,
        )
        .execute(&self.pool)
        .await?;
        if updated.rows_affected() == 1 {
            Ok(())
        } else {
            Err(RecordError::Unavailable(
                "the game is not in its lobby".to_owned(),
            ))
        }
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

    /// Notes that an unfinished game is gone for good: the server dropped it, or its host closed it.
    pub(crate) async fn mark_expired(&self, id: GameId) -> Result<(), RecordError> {
        sqlx::query!(
            "UPDATE games SET status = 'expired' WHERE id = $1 AND status IN ('lobby', 'running')",
            id.uuid(),
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Notes that the game could not be loaded: `detail` is the operator's account of why, `reason`
    /// the sentence its players read.
    pub(crate) async fn mark_failed(
        &self,
        id: GameId,
        detail: &str,
        reason: &str,
    ) -> Result<(), RecordError> {
        let detail: String = detail.chars().take(MAX_FAILURE_CHARS).collect();
        let reason: String = reason.chars().take(MAX_FAILURE_CHARS).collect();
        sqlx::query!(
            "UPDATE games SET status = 'failed', failure = $2, failure_reason = $3
             WHERE id = $1 AND status IN ('lobby', 'running')",
            id.uuid(),
            detail,
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
    //! Goal: every status, visibility and seat state has one stored spelling that parses back, unknown spellings are refused, a game is invite-only unless it says otherwise, and a database age never panics.
    use super::*;
    use rstest::rstest;

    #[rstest]
    #[case(Visibility::Public, "public")]
    #[case(Visibility::Invite, "invite")]
    fn visibilities_round_trip_and_serialize_as_stored(
        #[case] visibility: Visibility,
        #[case] text: &str,
    ) {
        assert_eq!(visibility.as_str(), text);
        assert_eq!(parse_visibility(text), Ok(visibility));
        let json = serde_json::to_string(&visibility).unwrap();
        assert_eq!(json, format!("\"{text}\""));
        assert_eq!(
            serde_json::from_str::<Visibility>(&json).unwrap(),
            visibility
        );
    }

    #[test]
    fn a_game_is_invite_only_unless_it_says_otherwise() {
        assert_eq!(Visibility::default(), Visibility::Invite);
    }

    #[rstest]
    #[case(SeatState::Issued, "issued")]
    #[case(SeatState::Open, "open")]
    #[case(SeatState::Claimed, "claimed")]
    fn seat_states_round_trip(#[case] state: SeatState, #[case] text: &str) {
        assert_eq!(state.as_str(), text);
        assert_eq!(SeatState::parse(text), Ok(state));
    }

    #[rstest]
    #[case("")]
    #[case("Public")]
    #[case("private")]
    fn unknown_visibilities_and_seat_states_are_corrupt(#[case] text: &str) {
        assert!(matches!(
            parse_visibility(text),
            Err(RecordError::Corrupt(_))
        ));
        assert!(matches!(
            SeatState::parse(text),
            Err(RecordError::Corrupt(_))
        ));
    }

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
