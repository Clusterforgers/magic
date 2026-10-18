//! Seats and accounts: binding a seat to a user, and listing the games a user has a seat in.

use std::collections::HashMap;

use mtg_core::ids::PlayerId;
use mtg_engine::state::GameResult;
use uuid::Uuid;

use super::{GameRecords, RecordError, RecordStatus};
use crate::game_id::GameId;
use crate::user_id::UserId;

/// What binding a seat to an account came to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Binding {
    /// The seat is now the account's.
    Bound,
    /// The seat already was the account's.
    Already,
    /// Another account holds the seat; a bound seat never changes hands.
    Taken,
    /// The account holds another seat of this game, and an account holds only one.
    Elsewhere,
    /// There is no such seat on record, as for a game that was never recorded.
    Missing,
}

/// One seat of a listed game.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UserGameSeat {
    /// The seat number.
    pub seat: usize,
    /// The name the seat was given at creation.
    pub name: String,
    /// The username of the account the seat is bound to, if any.
    pub user: Option<String>,
}

/// A game a user has a seat in.
#[derive(Debug, Clone, PartialEq)]
pub struct UserGame {
    /// The game.
    pub id: GameId,
    /// The user's seat.
    pub seat: usize,
    /// `Lobby`, `Running` or `Finished`; dropped and damaged games are not listed.
    pub status: RecordStatus,
    /// When the game was created, as `YYYY-MM-DDTHH:MM:SSZ` (UTC).
    pub created_at: String,
    /// How a finished game ended.
    pub result: Option<GameResult>,
    /// Every seat of the game in seat order, the user's own included.
    pub seats: Vec<UserGameSeat>,
}

impl GameRecords {
    /// Binds `seat` of `game` to `user` if nobody holds the seat and the user holds no other seat
    /// of the game. The database decides, so two accounts racing for a seat get one winner.
    pub(crate) async fn bind_user(
        &self,
        game: GameId,
        seat: PlayerId,
        user: UserId,
    ) -> Result<Binding, RecordError> {
        let number = i32::try_from(seat.index()).unwrap_or(i32::MAX);
        let updated = sqlx::query!(
            "UPDATE seats SET user_id = $3
             WHERE game_id = $1 AND seat = $2 AND user_id IS NULL
               AND NOT EXISTS (SELECT 1 FROM seats o WHERE o.game_id = $1 AND o.user_id = $3)",
            game.uuid(),
            number,
            user.uuid(),
        )
        .execute(&self.pool)
        .await;
        match updated {
            Ok(done) if done.rows_affected() == 1 => return Ok(Binding::Bound),
            Ok(_) => {}
            // Another request bound this user to another seat of the game a moment ago.
            Err(sqlx::Error::Database(error)) if error.is_unique_violation() => {
                return Ok(Binding::Elsewhere);
            }
            Err(error) => return Err(error.into()),
        }
        // Nothing changed; say why.
        let holder = sqlx::query!(
            "SELECT user_id FROM seats WHERE game_id = $1 AND seat = $2",
            game.uuid(),
            number,
        )
        .fetch_optional(&self.pool)
        .await?;
        Ok(match holder {
            None => Binding::Missing,
            Some(row) => match row.user_id {
                Some(holder) if holder == user.uuid() => Binding::Already,
                Some(_) => Binding::Taken,
                None => Binding::Elsewhere,
            },
        })
    }

    /// The games `user` has a seat in that are in their lobby, running or finished, newest first,
    /// at most `limit` of them, each with all its seats.
    pub async fn list_for_user(
        &self,
        user: UserId,
        limit: usize,
    ) -> Result<Vec<UserGame>, RecordError> {
        let limit = i64::try_from(limit).unwrap_or(i64::MAX);
        let games = sqlx::query!(
            r#"SELECT g.id, s.seat, g.status, g.result,
                      to_char(g.created_at AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS"Z"')
                          AS "created_at!"
               FROM seats s JOIN games g ON g.id = s.game_id
               WHERE s.user_id = $1 AND g.status IN ('lobby', 'running', 'finished')
               ORDER BY g.created_at DESC, g.id DESC
               LIMIT $2"#,
            user.uuid(),
            limit,
        )
        .fetch_all(&self.pool)
        .await?;

        let ids: Vec<Uuid> = games.iter().map(|game| game.id).collect();
        let rows = sqlx::query!(
            r#"SELECT s.game_id, s.seat, s.name, u.username AS "user?"
               FROM seats s LEFT JOIN users u ON u.id = s.user_id
               WHERE s.game_id = ANY($1)
               ORDER BY s.game_id, s.seat"#,
            &ids[..],
        )
        .fetch_all(&self.pool)
        .await?;
        let mut seats: HashMap<Uuid, Vec<UserGameSeat>> = HashMap::new();
        for row in rows {
            let Ok(seat) = usize::try_from(row.seat) else {
                continue;
            };
            seats.entry(row.game_id).or_default().push(UserGameSeat {
                seat,
                name: row.name,
                user: row.user,
            });
        }

        games
            .into_iter()
            .filter_map(|game| {
                let seat = usize::try_from(game.seat).ok()?;
                Some((game, seat))
            })
            .map(|(game, seat)| {
                Ok(UserGame {
                    id: GameId(game.id),
                    seat,
                    status: RecordStatus::parse(&game.status)?,
                    created_at: game.created_at,
                    // A result this server cannot read only matters for display.
                    result: game.result.and_then(|v| serde_json::from_value(v).ok()),
                    seats: seats.remove(&game.id).unwrap_or_default(),
                })
            })
            .collect()
    }
}
