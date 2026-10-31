//! Read-tier drafts: the whole rules files of cards the checker cannot prove, which `mtg-cli compile`
//! writes and the server reads next to the compiled rules. A draft is retired, never deleted, so a game
//! that started on it resumes; a card has at most one draft that is not retired, its current draft.

use mtg_import::ids::OracleId;
use serde_json::Value;
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::DbError;

/// A draft named by its card and read hash.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct DraftKey {
    /// The card.
    pub oracle_id: OracleId,
    /// The read hash of the draft: 32 lower-case hex characters.
    pub read_hash: String,
}

/// A draft to store.
#[derive(Debug, Clone, PartialEq)]
pub struct NewDraft {
    /// The card.
    pub oracle_id: OracleId,
    /// The read hash of the draft: 32 lower-case hex characters.
    pub read_hash: String,
    /// The draft's rules as the DSL's JSON, an object.
    pub rules: Value,
}

/// A stored draft.
#[derive(Debug, Clone, PartialEq)]
pub struct StoredDraft {
    /// The card.
    pub oracle_id: OracleId,
    /// The read hash of the draft.
    pub read_hash: String,
    /// The draft's rules as the DSL's JSON.
    pub rules: Value,
    /// Whether a later draft (or the build) replaced it.
    pub retired: bool,
}

impl StoredDraft {
    /// The card and read hash of the draft.
    pub fn key(&self) -> DraftKey {
        DraftKey {
            oracle_id: self.oracle_id.clone(),
            read_hash: self.read_hash.clone(),
        }
    }
}

/// What the current drafts and the verdicts looked like when [`DraftRepo::version`] read them: two equal
/// versions mean no draft became current or retired and no verdict was added or removed in between, so a
/// library built from them is current.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DraftsVersion {
    current: String,
    reads: i64,
    last_read: Option<i64>,
}

/// Reads and writes the drafts.
#[derive(Debug, Clone)]
pub struct DraftRepo {
    pool: PgPool,
}

/// The oracle id as the uuid column takes it; one that is no uuid names no card.
pub(crate) fn uuid_of(id: &OracleId) -> Option<Uuid> {
    Uuid::parse_str(id.as_str()).ok()
}

/// The keys as the two arrays a query unnests; a key whose id is no uuid names no draft and is left out.
pub(crate) fn unzip_keys(keys: &[DraftKey]) -> (Vec<Uuid>, Vec<String>) {
    keys.iter()
        .filter_map(|key| Some((uuid_of(&key.oracle_id)?, key.read_hash.clone())))
        .unzip()
}

impl DraftRepo {
    pub(crate) fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Makes each draft the current draft of its card, in one transaction: the card's other current draft
    /// is retired, a draft that is stored and retired becomes current again, one that is stored and
    /// current stays as it is (its rules are not rewritten: the hash covers them). Returns how many
    /// drafts became current. When a card appears twice, the last one wins.
    ///
    /// A draft for a card the database does not hold, or with a malformed hash, is an error and nothing
    /// is written.
    pub async fn insert(&self, drafts: &[NewDraft]) -> Result<u64, DbError> {
        let mut tx = self.pool.begin().await?;
        let mut became_current = 0;
        for draft in drafts {
            let Some(oracle_id) = uuid_of(&draft.oracle_id) else {
                return Err(DbError::Corrupt {
                    column: "card_drafts.oracle_id",
                    value: draft.oracle_id.as_str().to_owned(),
                });
            };
            sqlx::query!(
                "UPDATE card_drafts SET retired = true
                 WHERE oracle_id = $1 AND read_hash <> $2 AND NOT retired",
                oracle_id,
                draft.read_hash
            )
            .execute(&mut *tx)
            .await?;
            let done = sqlx::query!(
                "INSERT INTO card_drafts (oracle_id, read_hash, rules) VALUES ($1, $2, $3)
                 ON CONFLICT (oracle_id, read_hash) DO UPDATE SET retired = false
                 WHERE card_drafts.retired",
                oracle_id,
                draft.read_hash,
                draft.rules
            )
            .execute(&mut *tx)
            .await?;
            became_current += done.rows_affected();
        }
        tx.commit().await?;
        Ok(became_current)
    }

    /// Retires these drafts, which stay stored with their verdicts; returns how many were current.
    pub async fn retire(&self, drafts: &[DraftKey]) -> Result<u64, DbError> {
        let (ids, hashes) = unzip_keys(drafts);
        let done = sqlx::query!(
            "UPDATE card_drafts d SET retired = true
             FROM unnest($1::uuid[], $2::text[]) AS k(oracle_id, read_hash)
             WHERE d.oracle_id = k.oracle_id AND d.read_hash = k.read_hash AND NOT d.retired",
            &ids,
            &hashes
        )
        .execute(&self.pool)
        .await?;
        Ok(done.rows_affected())
    }

    /// The current draft of every card that has one, by oracle id.
    pub async fn current(&self) -> Result<Vec<StoredDraft>, DbError> {
        let rows = sqlx::query!(
            "SELECT oracle_id, read_hash, rules, retired FROM card_drafts
             WHERE NOT retired ORDER BY oracle_id"
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .into_iter()
            .map(|row| StoredDraft {
                oracle_id: OracleId(row.oracle_id.to_string()),
                read_hash: row.read_hash,
                rules: row.rules,
                retired: row.retired,
            })
            .collect())
    }

    /// The stored drafts with these keys, retired ones too (a game that started on a draft is built from
    /// it whatever has replaced it since), by oracle id and then age. A key that names no draft is left out.
    pub async fn get(&self, drafts: &[DraftKey]) -> Result<Vec<StoredDraft>, DbError> {
        let (ids, hashes) = unzip_keys(drafts);
        let rows = sqlx::query!(
            "SELECT d.oracle_id, d.read_hash, d.rules, d.retired FROM card_drafts d
             JOIN unnest($1::uuid[], $2::text[]) AS k(oracle_id, read_hash)
               ON d.oracle_id = k.oracle_id AND d.read_hash = k.read_hash
             ORDER BY d.oracle_id, d.created_at, d.read_hash",
            &ids,
            &hashes
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .into_iter()
            .map(|row| StoredDraft {
                oracle_id: OracleId(row.oracle_id.to_string()),
                read_hash: row.read_hash,
                rules: row.rules,
                retired: row.retired,
            })
            .collect())
    }

    /// A cheap digest of the current drafts and the verdicts, to tell whether a library built from them
    /// earlier is still current. A verdict on a retired draft moves it too, which only costs a rebuild.
    pub async fn version(&self) -> Result<DraftsVersion, DbError> {
        let row = sqlx::query!(
            r#"SELECT (SELECT md5(coalesce(string_agg(oracle_id::text || read_hash, ',' ORDER BY oracle_id), ''))
                       FROM card_drafts WHERE NOT retired) AS "current!",
                      (SELECT count(*) FROM card_reads) AS "reads!",
                      (SELECT max(id) FROM card_reads) AS last_read"#
        )
        .fetch_one(&self.pool)
        .await?;
        Ok(DraftsVersion {
            current: row.current,
            reads: row.reads,
            last_read: row.last_read,
        })
    }
}

#[cfg(test)]
mod tests {
    //! Goal: a key whose id is no uuid names no draft, and the keys of a query split into the two arrays it unnests in order.
    use super::*;

    fn key(id: &str, hash: &str) -> DraftKey {
        DraftKey {
            oracle_id: OracleId(id.into()),
            read_hash: hash.into(),
        }
    }

    #[test]
    fn keys_split_in_order_and_drop_ids_that_are_no_uuid() {
        let a = "4457ed35-7c10-48c8-9776-456485fdf070";
        let b = "00000000-0000-4000-8000-000000000001";
        let keys = [key(a, "h1"), key("not-an-id", "h2"), key(b, "h3")];
        let (ids, hashes) = unzip_keys(&keys);
        assert_eq!(
            ids,
            [Uuid::parse_str(a).unwrap(), Uuid::parse_str(b).unwrap()]
        );
        assert_eq!(hashes, ["h1", "h3"]);
        assert_eq!(unzip_keys(&[]), (vec![], vec![]));
    }
}
