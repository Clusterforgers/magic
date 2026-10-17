//! Error types for database access and imports.

use thiserror::Error;

use crate::runs::RunId;

/// Failure talking to Postgres or interpreting what it returned.
#[derive(Debug, Error)]
pub enum DbError {
    /// A connection or query failed.
    #[error("database error: {0}")]
    Query(#[from] sqlx::Error),
    /// Applying migrations failed.
    #[error("migration failed: {0}")]
    Migrate(#[from] sqlx::migrate::MigrateError),
    /// A JSON payload could not be produced or read.
    #[error("invalid JSON payload: {0}")]
    Json(#[from] serde_json::Error),
    /// A stored value is outside the domain this crate understands.
    #[error("stored {column} value {value:?} is not valid")]
    Corrupt {
        /// Column the value came from.
        column: &'static str,
        /// The offending value.
        value: String,
    },
    /// An import run was finished or failed although it was not running.
    #[error("import run {0} is not running")]
    RunNotRunning(RunId),
}
