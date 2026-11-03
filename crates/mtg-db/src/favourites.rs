//! Favourite precons: the precons an account starred, one row per account and precon code. A favourite
//! is a pointer to the precon, never a copy of its cards. Every per-account query names the account, so
//! one account never reads or removes another's favourite.

use sqlx::{PgConnection, PgPool};
use uuid::Uuid;

use crate::error::DbError;

/// The longest precon code the table takes, in characters.
pub const MAX_CODE_CHARS: usize = 120;

/// Whether `code` has the shape of a precon code: lower-case ASCII letters and digits joined by hyphens.
pub fn is_code(code: &str) -> bool {
    !code.is_empty()
        && code.len() <= MAX_CODE_CHARS
        && !code.starts_with('-')
        && code
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

/// Reads and writes favourite precons.
#[derive(Debug, Clone)]
pub struct FavouritePreconRepo {
    pool: PgPool,
}

impl FavouritePreconRepo {
    pub(crate) fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Stars `code` for `user`; starring it again changes nothing. A code of another shape (see [`is_code`])
    /// is refused with `false`, since the table would refuse it; true once the precon is a favourite.
    pub async fn add(&self, user: Uuid, code: &str) -> Result<bool, DbError> {
        if !is_code(code) {
            return Ok(false);
        }
        sqlx::query!(
            "INSERT INTO favourite_precons (user_id, code) VALUES ($1, $2)
             ON CONFLICT (user_id, code) DO NOTHING",
            user,
            code
        )
        .execute(&self.pool)
        .await?;
        Ok(true)
    }

    /// Takes the star of `code` back; false if `user` had not starred it.
    pub async fn remove(&self, user: Uuid, code: &str) -> Result<bool, DbError> {
        let removed = sqlx::query!(
            "DELETE FROM favourite_precons WHERE user_id = $1 AND code = $2",
            user,
            code
        )
        .execute(&self.pool)
        .await?;
        Ok(removed.rows_affected() > 0)
    }

    /// The codes `user` starred, the newest first.
    pub async fn list(&self, user: Uuid) -> Result<Vec<String>, DbError> {
        let rows = sqlx::query_scalar!(
            "SELECT code FROM favourite_precons WHERE user_id = $1 ORDER BY created_at DESC, code",
            user
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    /// Deletes every favourite of `user`, inside the caller's transaction `tx`, and returns how many went.
    /// A database whose favourites table is not migrated yet (the server's accounts can run without card
    /// data) holds none: 0.
    pub async fn delete_all_of(tx: &mut PgConnection, user: Uuid) -> Result<u64, DbError> {
        let migrated = sqlx::query_scalar!(
            r#"SELECT to_regclass('favourite_precons') IS NOT NULL AS "migrated!""#
        )
        .fetch_one(&mut *tx)
        .await?;
        if !migrated {
            return Ok(0);
        }
        let deleted = sqlx::query!("DELETE FROM favourite_precons WHERE user_id = $1", user)
            .execute(&mut *tx)
            .await?;
        Ok(deleted.rows_affected())
    }
}

#[cfg(test)]
mod tests {
    //! Goal: a precon code is lower-case words joined by hyphens, at most 120 characters, and anything else is no code.
    use rstest::rstest;

    use super::*;

    #[rstest]
    #[case("c18-adaptive-enchantment", true)]
    #[case("st", true)]
    #[case("c19-merciless-rage-4f3a", true)]
    #[case("", false)]
    #[case("-lead", false)]
    #[case("Upper", false)]
    #[case("with space", false)]
    #[case("under_score", false)]
    #[case("precon:c18", false)]
    #[case("é", false)]
    fn codes_have_one_shape(#[case] code: &str, #[case] valid: bool) {
        assert_eq!(is_code(code), valid, "{code}");
    }

    #[test]
    fn the_length_limit_is_the_table_s() {
        assert!(is_code(&"a".repeat(MAX_CODE_CHARS)));
        assert!(!is_code(&"a".repeat(MAX_CODE_CHARS + 1)));
    }
}
