//! Bookkeeping of import runs: when they started, how they ended and what they changed.

use std::time::Duration;

use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::PgPool;

use crate::error::DbError;

/// How long a run may stay `running` before the next import start marks it failed.
pub const STALE_AFTER: Duration = Duration::from_hours(1);

/// Why a reaped run failed.
const REAPED: &str = "abandoned: the importing process ended without closing the run";

/// Replaces NUL bytes, which Postgres text cannot hold, with a visible mark.
pub(crate) fn without_nul(text: &str) -> String {
    text.replace('\0', "\u{2400}")
}

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
    /// Scryfall Oracle Cards bulk data: the cards and one printing of each.
    ScryfallCards,
    /// Scryfall Default Cards bulk data: every printing.
    ScryfallPrintings,
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
    /// What the run read, as its starter named it (the CLI uses file checksums).
    pub input: Option<String>,
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
        self.start_with_input(source, None).await
    }

    /// Opens a new run in the `running` state that records what it reads (see [`ImportRun::input`]).
    pub async fn start_with_input(
        &self,
        source: ImportSource,
        input: Option<&str>,
    ) -> Result<RunId, DbError> {
        let id = sqlx::query_scalar!(
            r#"INSERT INTO import_runs (source, input) VALUES ($1, $2) RETURNING id AS "id: RunId""#,
            source as _,
            input
        )
        .fetch_one(&self.pool)
        .await?;
        Ok(id)
    }

    /// Marks every run that has been `running` for longer than `max_age` as failed and returns their ids.
    ///
    /// A run left `running` by a process that died never ends on its own; the importer calls this
    /// with [`STALE_AFTER`] before it opens a run.
    pub async fn reap_stale(&self, max_age: Duration) -> Result<Vec<RunId>, DbError> {
        let mut ids = sqlx::query_scalar!(
            r#"UPDATE import_runs
               SET status = 'failed', finished_at = now(), error = $2
               WHERE status = 'running' AND started_at < now() - make_interval(secs => $1)
               RETURNING id AS "id: RunId""#,
            max_age.as_secs_f64(),
            REAPED
        )
        .fetch_all(&self.pool)
        .await?;
        ids.sort();
        Ok(ids)
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
            without_nul(error),
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
                      status AS "status: _", counts, error, input
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
                      status AS "status: _", counts, error, input
               FROM import_runs WHERE source = $1
               ORDER BY started_at DESC, id DESC LIMIT 1"#,
            source as _
        )
        .fetch_optional(&self.pool)
        .await?;
        Ok(run)
    }

    /// The most recently started run of a source that succeeded.
    pub async fn latest_succeeded(
        &self,
        source: ImportSource,
    ) -> Result<Option<ImportRun>, DbError> {
        let run = sqlx::query_as!(
            ImportRun,
            r#"SELECT id AS "id: _", source AS "source: _", started_at, finished_at,
                      status AS "status: _", counts, error, input
               FROM import_runs WHERE source = $1 AND status = 'succeeded'
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
                      status AS "status: _", counts, error, input
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
