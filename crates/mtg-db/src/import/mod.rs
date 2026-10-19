//! Bulk ingestion of cards, rulings and precon decks, each recorded as an import run.

mod batches;
mod card_payload;
mod cards;
mod check;
mod precon_resolve;
mod precons;
mod rulings;

use mtg_import::scryfall::ScryfallError;
use serde::Serialize;
use sqlx::PgPool;
use thiserror::Error;

pub use batches::{SkipLog, SkippedRecord};
pub use cards::{CardImportReport, UpsertCounts};
pub use check::{CheckReport, check_cards, check_precons, check_rulings};
pub use precon_resolve::UnresolvedReason;
pub use precons::{DeckIngest, PreconImportReport, UnresolvedCard};
pub use rulings::RulingImportReport;

use crate::error::DbError;
use crate::runs::{ImportRuns, ImportSource, RunId, STALE_AFTER};

/// Records written per transaction unless overridden.
pub const DEFAULT_BATCH_SIZE: usize = 500;

/// Why an import run ended early.
#[derive(Debug, Error)]
pub enum ImportError {
    /// The record stream failed in a way that ends it; the run was marked failed.
    #[error("import run {run_id} aborted: {source}")]
    Stream {
        /// The run that was marked failed.
        run_id: RunId,
        /// The stream failure.
        source: ScryfallError,
    },
    /// The database failed; the run was marked failed when it could still be reached.
    #[error("import run {run_id:?} hit a database error: {source}")]
    Database {
        /// The run, absent when it could not even be started.
        run_id: Option<RunId>,
        /// The database failure.
        source: DbError,
    },
}

/// Failure inside an import body, before it is attached to a run.
#[derive(Debug)]
enum StepError {
    Stream(ScryfallError),
    Db(DbError),
}

impl From<ScryfallError> for StepError {
    fn from(error: ScryfallError) -> Self {
        StepError::Stream(error)
    }
}

impl From<DbError> for StepError {
    fn from(error: DbError) -> Self {
        StepError::Db(error)
    }
}

impl From<sqlx::Error> for StepError {
    fn from(error: sqlx::Error) -> Self {
        StepError::Db(error.into())
    }
}

/// Writes cards, rulings and precon decks into Postgres.
#[derive(Debug, Clone)]
pub struct Importer {
    pool: PgPool,
    runs: ImportRuns,
    batch_size: usize,
    input: Option<String>,
}

impl Importer {
    pub(crate) fn new(pool: PgPool) -> Self {
        Self {
            runs: ImportRuns::new(pool.clone()),
            pool,
            batch_size: DEFAULT_BATCH_SIZE,
            input: None,
        }
    }

    /// Sets how many records go into one transaction (at least one).
    #[must_use]
    pub fn with_batch_size(mut self, batch_size: usize) -> Self {
        self.batch_size = batch_size.max(1);
        self
    }

    /// Names the data the import reads, which each run records as its [`crate::ImportRun::input`].
    #[must_use]
    pub fn with_input(mut self, input: impl Into<String>) -> Self {
        self.input = Some(input.into());
        self
    }

    /// Reaps runs left behind by dead processes, then opens a run for `source`.
    async fn start(&self, source: ImportSource) -> Result<RunId, ImportError> {
        let opened = async {
            self.runs.reap_stale(STALE_AFTER).await?;
            self.runs
                .start_with_input(source, self.input.as_deref())
                .await
        };
        opened.await.map_err(|source| ImportError::Database {
            run_id: None,
            source,
        })
    }

    /// Closes the run according to `outcome` and returns the report on success.
    async fn conclude<R: Serialize>(
        &self,
        run_id: RunId,
        report: R,
        outcome: Result<(), StepError>,
    ) -> Result<R, ImportError> {
        let error = match outcome {
            Ok(()) => {
                return match self.runs.finish(run_id, &report).await {
                    Ok(()) => Ok(report),
                    Err(source) => Err(ImportError::Database {
                        run_id: Some(run_id),
                        source,
                    }),
                };
            }
            Err(StepError::Stream(source)) => ImportError::Stream { run_id, source },
            Err(StepError::Db(source)) => ImportError::Database {
                run_id: Some(run_id),
                source,
            },
        };
        // If the database is unreachable the run stays `running`; the import error is the useful one.
        let _ = self.runs.fail(run_id, &error.to_string(), &report).await;
        Err(error)
    }
}
