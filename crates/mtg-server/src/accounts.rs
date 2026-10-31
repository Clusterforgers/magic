//! Accounts and sessions in Postgres: the `users` and `sessions` tables, and what goes with an
//! account when it is deleted.
//!
//! A password is stored as its Argon2id hash and a session as the SHA-256 of its cookie token, so
//! a copy of the database opens no account and no session. Run [`migrate`](crate::migrate) first.

use std::fmt;
use std::time::Duration;

use mtg_db::{DeckRepo, FixMarkRepo};
use sqlx::PgPool;
use thiserror::Error;

use crate::credentials::Username;
use crate::secret::TokenDigest;
use crate::user_id::UserId;

/// Why an account operation failed.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub(crate) enum AccountError {
    /// Another account has this username, ignoring case.
    #[error("that username is taken")]
    Taken,
    /// The database could not be reached or refused the statement.
    #[error("accounts unavailable: {0}")]
    Unavailable(String),
}

impl From<sqlx::Error> for AccountError {
    fn from(error: sqlx::Error) -> Self {
        AccountError::Unavailable(error.to_string())
    }
}

/// A signed-in or registering account, as other players may know it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct User {
    /// The account.
    pub(crate) id: UserId,
    /// The username as it was registered.
    pub(crate) username: String,
    /// Whether this is the admin account; read with the account, so a demotion shows on the next request.
    pub(crate) is_admin: bool,
}

/// One account of the admin's list, without anything that opens it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Listed {
    /// The account.
    pub(crate) id: UserId,
    /// The username as it was registered.
    pub(crate) username: String,
    /// When it was made, as `YYYY-MM-DDTHH:MM:SSZ` (UTC).
    pub(crate) created_at: String,
    /// Whether this is the admin account.
    pub(crate) is_admin: bool,
}

/// An account together with the hash its password is checked against.
#[derive(Clone, PartialEq, Eq)]
pub(crate) struct Stored {
    /// Who the hash belongs to.
    pub(crate) user: User,
    /// The Argon2id PHC string.
    pub(crate) password_hash: String,
}

impl fmt::Debug for Stored {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Stored")
            .field("user", &self.user)
            .field("password_hash", &"<redacted>")
            .finish()
    }
}

/// The `users` and `sessions` tables; clones share the pool.
#[derive(Debug, Clone)]
pub(crate) struct Accounts {
    pool: PgPool,
}

impl Accounts {
    /// Accounts over `pool`.
    pub(crate) fn new(pool: PgPool) -> Accounts {
        Accounts { pool }
    }

    /// Stores a new account; [`AccountError::Taken`] if the username is in use, ignoring case.
    pub(crate) async fn create_user(
        &self,
        name: &Username,
        password_hash: &str,
    ) -> Result<User, AccountError> {
        let id = UserId::random();
        let inserted = sqlx::query!(
            "INSERT INTO users (id, username, username_key, password_hash) VALUES ($1, $2, $3, $4)",
            id.uuid(),
            name.display,
            name.key,
            password_hash,
        )
        .execute(&self.pool)
        .await;
        match inserted {
            Ok(_) => Ok(User {
                id,
                username: name.display.clone(),
                is_admin: false,
            }),
            Err(sqlx::Error::Database(error)) if error.is_unique_violation() => {
                Err(AccountError::Taken)
            }
            Err(error) => Err(error.into()),
        }
    }

    /// The account whose lower-case username is `key`, with its password hash.
    pub(crate) async fn find(&self, key: &str) -> Result<Option<Stored>, AccountError> {
        let row = sqlx::query!(
            "SELECT id, username, is_admin, password_hash FROM users WHERE username_key = $1",
            key,
        )
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(|row| Stored {
            user: User {
                id: UserId(row.id),
                username: row.username,
                is_admin: row.is_admin,
            },
            password_hash: row.password_hash,
        }))
    }

    /// Makes the account whose lower-case username is `name` the admin and every other account not, in one
    /// statement, so at most one admin exists; with `None` nobody is. Returns how many admins there are now: 0
    /// when `name` belongs to no account.
    pub(crate) async fn set_admin(&self, name: Option<&str>) -> Result<i64, AccountError> {
        let row = sqlx::query!(
            r#"WITH updated AS (
                   UPDATE users SET is_admin = (username_key = lower(coalesce($1::text, '')))
                   RETURNING is_admin
               )
               SELECT count(*) FILTER (WHERE is_admin) AS "admins!" FROM updated"#,
            name,
        )
        .fetch_one(&self.pool)
        .await?;
        Ok(row.admins)
    }

    /// Every account, oldest first.
    pub(crate) async fn list_users(&self) -> Result<Vec<Listed>, AccountError> {
        let rows = sqlx::query!(
            r#"SELECT id, username, is_admin,
                      to_char(created_at AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS"Z"') AS "created_at!"
               FROM users ORDER BY created_at, username_key"#,
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .into_iter()
            .map(|row| Listed {
                id: UserId(row.id),
                username: row.username,
                created_at: row.created_at,
                is_admin: row.is_admin,
            })
            .collect())
    }

    /// The Argon2id hash of `user`'s password; `None` for an account that is gone.
    pub(crate) async fn password_hash(&self, user: UserId) -> Result<Option<String>, AccountError> {
        let row = sqlx::query!("SELECT password_hash FROM users WHERE id = $1", user.uuid())
            .fetch_optional(&self.pool)
            .await?;
        Ok(row.map(|row| row.password_hash))
    }

    /// Replaces `user`'s password hash and ends every session of the account but `keep`, in one
    /// transaction. False, and nothing changed, for an account that is gone.
    pub(crate) async fn change_password(
        &self,
        user: UserId,
        password_hash: &str,
        keep: &TokenDigest,
    ) -> Result<bool, AccountError> {
        let mut tx = self.pool.begin().await?;
        let changed = sqlx::query!(
            "UPDATE users SET password_hash = $2 WHERE id = $1",
            user.uuid(),
            password_hash,
        )
        .execute(&mut *tx)
        .await?;
        if changed.rows_affected() == 0 {
            return Ok(false);
        }
        sqlx::query!(
            "DELETE FROM sessions WHERE user_id = $1 AND token_digest <> $2",
            user.uuid(),
            &keep.as_bytes()[..],
        )
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(true)
    }

    /// Deletes the account with everything that refers to it, in one transaction: its decks, its fix
    /// marks, its sessions (by cascade) and its hold on seats (a seat stays and loses its account). False
    /// for an account that is already gone.
    pub(crate) async fn delete_user(&self, user: UserId) -> Result<bool, AccountError> {
        let mut tx = self.pool.begin().await?;
        DeckRepo::delete_all_of(&mut tx, user.uuid())
            .await
            .map_err(|error| AccountError::Unavailable(error.to_string()))?;
        FixMarkRepo::delete_all_of(&mut tx, user.uuid())
            .await
            .map_err(|error| AccountError::Unavailable(error.to_string()))?;
        let deleted = sqlx::query!("DELETE FROM users WHERE id = $1", user.uuid())
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(deleted.rows_affected() == 1)
    }

    /// Stores a session for `user` that ends `lifetime` from now, by the database's clock.
    pub(crate) async fn start_session(
        &self,
        digest: &TokenDigest,
        user: UserId,
        lifetime: Duration,
    ) -> Result<(), AccountError> {
        sqlx::query!(
            "INSERT INTO sessions (token_digest, user_id, expires_at)
             VALUES ($1, $2, now() + make_interval(secs => $3))",
            &digest.as_bytes()[..],
            user.uuid(),
            lifetime.as_secs_f64(),
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// The account a live session belongs to; `None` for an unknown or expired one.
    pub(crate) async fn session_user(
        &self,
        digest: &TokenDigest,
    ) -> Result<Option<User>, AccountError> {
        let row = sqlx::query!(
            "SELECT u.id, u.username, u.is_admin
             FROM sessions s JOIN users u ON u.id = s.user_id
             WHERE s.token_digest = $1 AND s.expires_at > now()",
            &digest.as_bytes()[..],
        )
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(|row| User {
            id: UserId(row.id),
            username: row.username,
            is_admin: row.is_admin,
        }))
    }

    /// Ends a session; one that does not exist is not an error.
    pub(crate) async fn end_session(&self, digest: &TokenDigest) -> Result<(), AccountError> {
        sqlx::query!(
            "DELETE FROM sessions WHERE token_digest = $1",
            &digest.as_bytes()[..],
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Deletes the sessions that have expired; returns how many.
    pub(crate) async fn purge_expired(&self) -> Result<u64, AccountError> {
        let purged = sqlx::query!("DELETE FROM sessions WHERE expires_at <= now()")
            .execute(&self.pool)
            .await?;
        Ok(purged.rows_affected())
    }
}

#[cfg(test)]
mod tests {
    //! Goal: what the account types print never includes a password hash.
    use super::*;

    #[test]
    fn debug_output_hides_the_password_hash() {
        let stored = Stored {
            user: User {
                id: UserId::random(),
                username: "ann".into(),
                is_admin: false,
            },
            password_hash: "$argon2id$v=19$m=8,t=1,p=1$c2FsdHNhbHQ$aGFzaGhhc2g".into(),
        };
        let shown = format!("{stored:?} {stored:#?}");
        assert!(
            !shown.contains("argon2id") && !shown.contains("aGFzaGhhc2g"),
            "{shown}"
        );
        assert!(shown.contains("ann") && shown.contains("<redacted>"));
    }
}
