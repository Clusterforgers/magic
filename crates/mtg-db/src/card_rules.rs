//! Compiled rules per card: the rows `mtg-cli compile` writes and the server reads next to the
//! hand-written rules files.

use chrono::{DateTime, Utc};
use mtg_import::ids::OracleId;
use serde_json::Value;
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::DbError;
use crate::model::ImplStatus;

/// One row to store.
#[derive(Debug, Clone, PartialEq)]
pub struct NewRules {
    /// The card.
    pub oracle_id: OracleId,
    /// Its rules as the DSL's JSON.
    pub rules: Value,
    /// How much of the card the rules cover: `Full` or `Partial`, never `Manual`.
    pub status: ImplStatus,
    /// Digest of the printed data the compiler read.
    pub fingerprint: String,
    /// Digest of the compiler that wrote the rules.
    pub compiler_version: String,
}

/// A stored row.
#[derive(Debug, Clone, PartialEq)]
pub struct StoredRules {
    /// The card.
    pub oracle_id: OracleId,
    /// Its rules as the DSL's JSON.
    pub rules: Value,
    /// How much of the card the rules cover.
    pub status: ImplStatus,
    /// Digest of the printed data the compiler read.
    pub fingerprint: String,
    /// Digest of the compiler that wrote the rules.
    pub compiler_version: String,
}

/// What a row says of its age, without the rules: enough to tell whether the compiler has to read the card again.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuleStamp {
    /// The card.
    pub oracle_id: OracleId,
    /// How much of the card the rules cover.
    pub status: ImplStatus,
    /// Digest of the printed data the compiler read.
    pub fingerprint: String,
    /// Digest of the compiler that wrote the rules.
    pub compiler_version: String,
}

/// How many rows hold each status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct RuleCounts {
    /// Rules that cover the whole text.
    pub full: u64,
    /// Rules with clauses left to the table.
    pub partial: u64,
}

/// What the stored rules looked like when [`CardRulesRepo::version`] read them: two equal versions mean no row was
/// added, removed or written again in between, so anything built from the rows is current.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RulesVersion {
    rows: i64,
    changed: Option<DateTime<Utc>>,
}

/// Reads and writes the compiled rules.
#[derive(Debug, Clone)]
pub struct CardRulesRepo {
    pool: PgPool,
}

fn uuid_of(id: &OracleId) -> Option<Uuid> {
    Uuid::parse_str(id.as_str()).ok()
}

impl CardRulesRepo {
    pub(crate) fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Stores the rows in one transaction, replacing the row of a card that has one; returns how many were written.
    ///
    /// A row for a card the database does not hold, or with the status `Manual`, is an error and nothing is written.
    pub async fn put(&self, rows: &[NewRules]) -> Result<u64, DbError> {
        let mut tx = self.pool.begin().await?;
        let mut written = 0;
        for row in rows {
            let Some(oracle_id) = uuid_of(&row.oracle_id) else {
                return Err(DbError::Corrupt {
                    column: "card_rules.oracle_id",
                    value: row.oracle_id.as_str().to_owned(),
                });
            };
            let done = sqlx::query!(
                r#"INSERT INTO card_rules (oracle_id, rules, status, fingerprint, compiler_version)
                   VALUES ($1, $2, $3, $4, $5)
                   ON CONFLICT (oracle_id) DO UPDATE
                   SET rules = EXCLUDED.rules, status = EXCLUDED.status, fingerprint = EXCLUDED.fingerprint,
                       compiler_version = EXCLUDED.compiler_version, updated_at = now()"#,
                oracle_id,
                row.rules,
                row.status as ImplStatus,
                row.fingerprint,
                row.compiler_version,
            )
            .execute(&mut *tx)
            .await?;
            written += done.rows_affected();
        }
        tx.commit().await?;
        Ok(written)
    }

    /// Removes the rows of these cards; returns how many there were.
    pub async fn remove(&self, ids: &[OracleId]) -> Result<u64, DbError> {
        let ids: Vec<Uuid> = ids.iter().filter_map(uuid_of).collect();
        let done = sqlx::query!("DELETE FROM card_rules WHERE oracle_id = ANY($1)", &ids)
            .execute(&self.pool)
            .await?;
        Ok(done.rows_affected())
    }

    /// Every row, by oracle id.
    pub async fn all(&self) -> Result<Vec<StoredRules>, DbError> {
        let rows = sqlx::query!(
            r#"SELECT oracle_id, rules, status AS "status: ImplStatus", fingerprint, compiler_version
               FROM card_rules ORDER BY oracle_id"#
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .into_iter()
            .map(|row| StoredRules {
                oracle_id: OracleId(row.oracle_id.to_string()),
                rules: row.rules,
                status: row.status,
                fingerprint: row.fingerprint,
                compiler_version: row.compiler_version,
            })
            .collect())
    }

    /// The age of every row, by oracle id.
    pub async fn stamps(&self) -> Result<Vec<RuleStamp>, DbError> {
        let rows = sqlx::query!(
            r#"SELECT oracle_id, status AS "status: ImplStatus", fingerprint, compiler_version
               FROM card_rules ORDER BY oracle_id"#
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .into_iter()
            .map(|row| RuleStamp {
                oracle_id: OracleId(row.oracle_id.to_string()),
                status: row.status,
                fingerprint: row.fingerprint,
                compiler_version: row.compiler_version,
            })
            .collect())
    }

    /// A cheap fingerprint of the rows, to tell whether rules built from them earlier are still current.
    pub async fn version(&self) -> Result<RulesVersion, DbError> {
        let row = sqlx::query!(
            r#"SELECT count(*) AS "rows!", max(updated_at) AS changed FROM card_rules"#
        )
        .fetch_one(&self.pool)
        .await?;
        Ok(RulesVersion {
            rows: row.rows,
            changed: row.changed,
        })
    }

    /// How many rows hold each status.
    pub async fn counts(&self) -> Result<RuleCounts, DbError> {
        let row = sqlx::query!(
            r#"SELECT count(*) FILTER (WHERE status = 'full') AS "full!",
                      count(*) FILTER (WHERE status = 'partial') AS "partial!"
               FROM card_rules"#
        )
        .fetch_one(&self.pool)
        .await?;
        Ok(RuleCounts {
            full: row.full.unsigned_abs(),
            partial: row.partial.unsigned_abs(),
        })
    }
}
