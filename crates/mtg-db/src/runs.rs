//! Bookkeeping of import runs: when they started, how they ended and what they changed.

use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::PgPool;

use crate::error::DbError;

/// Identifier of an import run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, sqlx::Type)]
#[serde(transparent)]
#[sqlx(transparent)]
pub struct RunId(pub i64);

impl std::fmt::Display for RunId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

/// What an import run reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, sqlx::Type)]
#[sqlx(type_name = "import_source", rename_all = "snake_case")]
pub enum ImportSource {
    /// Scryfall Oracle Cards or Default Cards bulk data.
    ScryfallCards,
    /// Scryfall rulings bulk data.
    ScryfallRulings,
    /// MTGJSON precon deck files.
    MtgjsonDecks,
}

/// Where an import run stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, sqlx::Type)]
#[sqlx(type_name = "import_status", rename_all = "lowercase")]
pub enum RunStatus {
    /// Still importing.
    Running,
    /// Finished without a fatal error.
    Succeeded,
    /// Aborted by a fatal error.
    Failed,
}

/// A row of the import history.
#[derive(Debug, Clone, PartialEq)]
pub struct ImportRun {
    /// Run identifier.
    pub id: RunId,
    /// What the run imports.
    pub source: ImportSource,
    /// When the run started.
    pub started_at: DateTime<Utc>,
    /// When the run ended; `None` while running.
    pub finished_at: Option<DateTime<Utc>>,
    /// Current status.
    pub status: RunStatus,
    /// Counters reported by the run, as JSON.
    pub counts: serde_json::Value,
    /// Failure message of a failed run.
    pub error: Option<String>,
}

/// Records and reads the history of import runs.
#[derive(Debug, Clone)]
pub struct ImportRuns {
    pool: PgPool,
}

impl ImportRuns {
    pub(crate) fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Opens a new run in the `running` state.
    pub async fn start(&self, source: ImportSource) -> Result<RunId, DbError> {
        let id = sqlx::query_scalar!(
            r#"INSERT INTO import_runs (source) VALUES ($1) RETURNING id AS "id: RunId""#,
            source as _
        )
        .fetch_one(&self.pool)
        .await?;
        Ok(id)
    }

    /// Marks a running run as succeeded and stores its counters.
    pub async fn finish(&self, id: RunId, counts: &impl Serialize) -> Result<(), DbError> {
        let counts = serde_json::to_value(counts)?;
        let done = sqlx::query!(
            "UPDATE import_runs
             SET status = 'succeeded', finished_at = now(), counts = $2
             WHERE id = $1 AND status = 'running'",
            id as _,
            counts
        )
        .execute(&self.pool)
        .await?;
        require_updated(id, done.rows_affected())
    }

    /// Marks a running run as failed with a message and the counters reached so far.
    pub async fn fail(
        &self,
        id: RunId,
        error: &str,
        counts: &impl Serialize,
    ) -> Result<(), DbError> {
        let counts = serde_json::to_value(counts)?;
        let done = sqlx::query!(
            "UPDATE import_runs
             SET status = 'failed', finished_at = now(), error = $2, counts = $3
             WHERE id = $1 AND status = 'running'",
            id as _,
            error,
            counts
        )
        .execute(&self.pool)
        .await?;
        require_updated(id, done.rows_affected())
    }

    /// Loads one run.
    pub async fn get(&self, id: RunId) -> Result<Option<ImportRun>, DbError> {
        let run = sqlx::query_as!(
            ImportRun,
            r#"SELECT id AS "id: _", source AS "source: _", started_at, finished_at,
                      status AS "status: _", counts, error
               FROM import_runs WHERE id = $1"#,
            id as _
        )
        .fetch_optional(&self.pool)
        .await?;
        Ok(run)
    }

    /// The most recently started run of a source.
    pub async fn latest(&self, source: ImportSource) -> Result<Option<ImportRun>, DbError> {
        let run = sqlx::query_as!(
            ImportRun,
            r#"SELECT id AS "id: _", source AS "source: _", started_at, finished_at,
                      status AS "status: _", counts, error
               FROM import_runs WHERE source = $1
               ORDER BY started_at DESC, id DESC LIMIT 1"#,
            source as _
        )
        .fetch_optional(&self.pool)
        .await?;
        Ok(run)
    }

    /// The `limit` most recently started runs of any source, newest first.
    pub async fn recent(&self, limit: u32) -> Result<Vec<ImportRun>, DbError> {
        let runs = sqlx::query_as!(
            ImportRun,
            r#"SELECT id AS "id: _", source AS "source: _", started_at, finished_at,
                      status AS "status: _", counts, error
               FROM import_runs
               ORDER BY started_at DESC, id DESC LIMIT $1"#,
            i64::from(limit)
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(runs)
    }
}

fn require_updated(id: RunId, rows: u64) -> Result<(), DbError> {
    if rows == 1 {
        Ok(())
    } else {
        Err(DbError::RunNotRunning(id))
    }
}
