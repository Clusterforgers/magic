//! Fix marks: the cards an account called important to fix, with an optional short note. A mark is
//! one row per account and card; the owner reads the marks ranked by how many accounts made them.
//! Every per-account query names the account, so one account never reads or removes another's mark.

use mtg_import::ids::OracleId;
use sqlx::{PgConnection, PgPool};
use uuid::Uuid;

use crate::error::DbError;

/// The longest note of a mark, in characters.
pub const MAX_NOTE_CHARS: usize = 280;

/// A mark an account made, with the name of its card.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FixMark {
    /// The marked card.
    pub oracle_id: OracleId,
    /// The card's name.
    pub name: String,
    /// What the account said is wrong, if anything.
    pub note: Option<String>,
    /// When the account marked the card, as `YYYY-MM-DDTHH:MM:SSZ` (UTC).
    pub created_at: String,
}

/// A card with the number of accounts that marked it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RankedMark {
    /// The marked card.
    pub oracle_id: OracleId,
    /// The card's name.
    pub name: String,
    /// How many accounts marked it.
    pub marks: u32,
    /// The note of the newest mark that has one.
    pub newest_note: Option<String>,
}

/// Reads and writes fix marks.
#[derive(Debug, Clone)]
pub struct FixMarkRepo {
    pool: PgPool,
}

/// The oracle id as the uuid column takes it; one that is no uuid names no card.
fn uuid_of(id: &OracleId) -> Option<Uuid> {
    Uuid::parse_str(id.as_str()).ok()
}

/// A note as it is stored: trimmed, and none when it is empty.
pub(crate) fn clean(note: Option<&str>) -> Option<&str> {
    note.map(str::trim).filter(|note| !note.is_empty())
}

/// The count of a `count(*)` as the unsigned number it is.
fn count_of(count: i64) -> u32 {
    u32::try_from(count).unwrap_or(u32::MAX)
}

impl FixMarkRepo {
    pub(crate) fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Marks `card` for `user`, or replaces the note of a mark that exists (its time stays). The note is
    /// trimmed and an empty one is none. `None` if no stored card has the id; a note over
    /// [`MAX_NOTE_CHARS`] is refused by the database as an error.
    pub async fn mark(
        &self,
        user: Uuid,
        card: &OracleId,
        note: Option<&str>,
    ) -> Result<Option<FixMark>, DbError> {
        let Some(oracle) = uuid_of(card) else {
            return Ok(None);
        };
        let row = sqlx::query!(
            r#"WITH written AS (
                   INSERT INTO card_fix_marks (user_id, oracle_id, note)
                   SELECT $1, c.oracle_id, $3 FROM cards c WHERE c.oracle_id = $2
                   ON CONFLICT (user_id, oracle_id) DO UPDATE SET note = EXCLUDED.note
                   RETURNING oracle_id, note,
                       to_char(created_at AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS"Z"') AS created_at
               )
               SELECT w.oracle_id AS "oracle_id!", c.name AS "name!", w.note, w.created_at AS "created_at!"
               FROM written w JOIN cards c ON c.oracle_id = w.oracle_id"#,
            user,
            oracle,
            clean(note)
        )
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(|row| FixMark {
            oracle_id: OracleId(row.oracle_id.to_string()),
            name: row.name,
            note: row.note,
            created_at: row.created_at,
        }))
    }

    /// Removes the mark `user` made on `card`; false if there was none.
    pub async fn unmark(&self, user: Uuid, card: &OracleId) -> Result<bool, DbError> {
        let Some(oracle) = uuid_of(card) else {
            return Ok(false);
        };
        let removed = sqlx::query!(
            "DELETE FROM card_fix_marks WHERE user_id = $1 AND oracle_id = $2",
            user,
            oracle
        )
        .execute(&self.pool)
        .await?;
        Ok(removed.rows_affected() > 0)
    }

    /// Every mark of `user`, the newest first.
    pub async fn list(&self, user: Uuid) -> Result<Vec<FixMark>, DbError> {
        let rows = sqlx::query!(
            r#"SELECT m.oracle_id, c.name, m.note,
                      to_char(m.created_at AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS"Z"') AS "created_at!"
               FROM card_fix_marks m JOIN cards c ON c.oracle_id = m.oracle_id
               WHERE m.user_id = $1 ORDER BY m.created_at DESC, c.name, m.oracle_id"#,
            user
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .into_iter()
            .map(|row| FixMark {
                oracle_id: OracleId(row.oracle_id.to_string()),
                name: row.name,
                note: row.note,
                created_at: row.created_at,
            })
            .collect())
    }

    /// The marked cards, the most marked first (ties by the newest mark, then name), each with its
    /// newest note; `limit` cuts the list, `None` returns every marked card.
    pub async fn ranked(&self, limit: Option<u32>) -> Result<Vec<RankedMark>, DbError> {
        let rows = sqlx::query!(
            r#"SELECT m.oracle_id AS "oracle_id!", c.name AS "name!", count(*) AS "marks!",
                      (array_agg(m.note ORDER BY m.created_at DESC) FILTER (WHERE m.note IS NOT NULL))[1] AS newest_note
               FROM card_fix_marks m JOIN cards c ON c.oracle_id = m.oracle_id
               GROUP BY m.oracle_id, c.name
               ORDER BY count(*) DESC, max(m.created_at) DESC, c.name, m.oracle_id
               LIMIT $1"#,
            limit.map(i64::from)
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .into_iter()
            .map(|row| RankedMark {
                oracle_id: OracleId(row.oracle_id.to_string()),
                name: row.name,
                marks: count_of(row.marks),
                newest_note: row.newest_note,
            })
            .collect())
    }

    /// Deletes every mark of `user`, inside the caller's transaction `tx`, and returns how many went.
    /// A database whose fix-mark table is not migrated yet (the server's accounts can run without
    /// card data) holds none: 0.
    pub async fn delete_all_of(tx: &mut PgConnection, user: Uuid) -> Result<u64, DbError> {
        let migrated = sqlx::query_scalar!(
            r#"SELECT to_regclass('card_fix_marks') IS NOT NULL AS "migrated!""#
        )
        .fetch_one(&mut *tx)
        .await?;
        if !migrated {
            return Ok(0);
        }
        let deleted = sqlx::query!("DELETE FROM card_fix_marks WHERE user_id = $1", user)
            .execute(&mut *tx)
            .await?;
        Ok(deleted.rows_affected())
    }
}

#[cfg(test)]
mod tests {
    //! Goal: a note is stored trimmed and an empty one is none, and an id that is no uuid names no card.
    use rstest::rstest;

    use super::*;

    #[rstest]
    #[case(None, None)]
    #[case(Some(""), None)]
    #[case(Some("  \n"), None)]
    #[case(Some(" it does not draw "), Some("it does not draw"))]
    fn notes_are_trimmed_and_empty_ones_are_none(
        #[case] note: Option<&str>,
        #[case] stored: Option<&str>,
    ) {
        assert_eq!(clean(note), stored);
    }

    #[test]
    fn an_id_that_is_no_uuid_names_no_card() {
        assert_eq!(uuid_of(&OracleId("not-an-id".into())), None);
        assert!(uuid_of(&OracleId("4457ed35-7c10-48c8-9776-456485fdf070".into())).is_some());
    }

    #[test]
    fn counts_saturate() {
        assert_eq!(count_of(3), 3);
        assert_eq!(count_of(-1), u32::MAX);
        assert_eq!(count_of(i64::MAX), u32::MAX);
    }
}
