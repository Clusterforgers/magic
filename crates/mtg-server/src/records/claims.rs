//! Seats that a join takes and gives back while the game is in its lobby, and the visibility toggle.

use mtg_core::ids::PlayerId;

use super::{GameRecords, RecordError, Visibility};
use crate::game_id::GameId;
use crate::user_id::UserId;

/// What taking an open seat for a visitor came to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Claimed {
    /// The seat is the visitor's now.
    Done,
    /// The seat is not open any more, or the game's lobby is over.
    NotOpen,
    /// The account already holds this seat of the game (an account holds one per game).
    Holds(usize),
}

impl GameRecords {
    /// Gives the open `seat` of a game in its lobby to a visitor: its name, the digest of the token
    /// made for it and the account, if the visitor is signed in. The row decides, so two claims for
    /// one seat get one winner.
    pub(crate) async fn claim_seat(
        &self,
        game: GameId,
        seat: PlayerId,
        name: &str,
        digest: &[u8; 32],
        user: Option<UserId>,
    ) -> Result<Claimed, RecordError> {
        let number = i32::try_from(seat.index()).unwrap_or(i32::MAX);
        let user = user.map(UserId::uuid);
        let updated = sqlx::query!(
            "UPDATE seats SET state = 'claimed', name = $3, token_digest = $4, user_id = $5,
                              joined = false
             WHERE game_id = $1 AND seat = $2 AND state = 'open'
               AND EXISTS (SELECT 1 FROM games g WHERE g.id = $1 AND g.status = 'lobby')",
            game.uuid(),
            number,
            name,
            digest.as_slice(),
            user,
        )
        .execute(&self.pool)
        .await;
        match updated {
            Ok(done) if done.rows_affected() == 1 => Ok(Claimed::Done),
            Ok(_) => Ok(Claimed::NotOpen),
            // The account took another seat of this game a moment ago.
            Err(sqlx::Error::Database(error)) if error.is_unique_violation() => {
                let Some(user) = user else {
                    return Err(sqlx::Error::Database(error).into());
                };
                let held = sqlx::query!(
                    "SELECT seat FROM seats WHERE game_id = $1 AND user_id = $2",
                    game.uuid(),
                    user,
                )
                .fetch_optional(&self.pool)
                .await?;
                Ok(match held.and_then(|row| usize::try_from(row.seat).ok()) {
                    Some(seat) => Claimed::Holds(seat),
                    None => Claimed::NotOpen,
                })
            }
            Err(error) => Err(error.into()),
        }
    }

    /// Takes a claimed seat of a game in its lobby back: it is open again, with no name, token or
    /// account. False if the seat was not claimed or the lobby is over.
    pub(crate) async fn release_seat(
        &self,
        game: GameId,
        seat: PlayerId,
    ) -> Result<bool, RecordError> {
        let number = i32::try_from(seat.index()).unwrap_or(i32::MAX);
        let updated = sqlx::query!(
            "UPDATE seats SET state = 'open', name = '', token_digest = NULL, user_id = NULL,
                              joined = false
             WHERE game_id = $1 AND seat = $2 AND state = 'claimed'
               AND EXISTS (SELECT 1 FROM games g WHERE g.id = $1 AND g.status = 'lobby')",
            game.uuid(),
            number,
        )
        .execute(&self.pool)
        .await?;
        Ok(updated.rows_affected() == 1)
    }

    /// Changes who may find the game. False if there is no such game.
    pub(crate) async fn set_visibility(
        &self,
        game: GameId,
        visibility: Visibility,
    ) -> Result<bool, RecordError> {
        let updated = sqlx::query!(
            "UPDATE games SET visibility = $2 WHERE id = $1",
            game.uuid(),
            visibility.as_str(),
        )
        .execute(&self.pool)
        .await?;
        Ok(updated.rows_affected() == 1)
    }
}
