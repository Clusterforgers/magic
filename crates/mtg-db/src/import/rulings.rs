//! Idempotent batched ingest of rulings for cards that are already stored.

use mtg_import::scryfall::{RulingRecord, ScryfallError};
use serde::Serialize;
use sqlx::types::Json;
use uuid::Uuid;

use super::batches::{Batches, SkipLog};
use super::card_payload::{Rejection, iso_date, reject_nul};
use super::{ImportError, Importer, StepError};
use crate::codec::ruling_source_keyword;
use crate::error::DbError;
use crate::runs::{ImportSource, RunId};

/// Outcome of a rulings import; counts are per batch.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RulingImportReport {
    /// The run that recorded this import.
    pub run_id: RunId,
    /// Rulings that were newly stored.
    pub inserted: u64,
    /// Rulings that were already stored.
    pub unchanged: u64,
    /// Rulings whose card is not in the database, so they were not stored.
    pub orphaned: u64,
    /// Records that were not imported.
    pub skipped: SkipLog,
}

/// A `rulings` row in the shape of the insert statement's `jsonb_to_recordset` columns.
#[derive(Debug, Serialize)]
pub(super) struct RulingPayload {
    // Preserves stream order, so ids of new rulings follow the file.
    seq: u64,
    oracle_id: Uuid,
    source: &'static str,
    published_at: Option<String>,
    comment: String,
}

pub(super) fn ruling_row(seq: u64, record: &RulingRecord) -> Result<RulingPayload, Rejection> {
    let oracle_id = Uuid::parse_str(record.oracle_id.as_str())
        .map_err(|_| Rejection::OracleId(record.oracle_id.to_string()))?;
    if record.comment.trim().is_empty() {
        return Err(Rejection::BlankComment);
    }
    let row = RulingPayload {
        seq,
        oracle_id,
        source: ruling_source_keyword(record.source),
        published_at: record.published_at.as_deref().map(iso_date).transpose()?,
        comment: record.comment.clone(),
    };
    reject_nul(&row)?;
    Ok(row)
}

impl Importer {
    /// Stores rulings from `records` in batches, recording the run.
    ///
    /// Rulings of unknown cards are counted as orphaned, so import cards first. Failure and
    /// iteration behave as in [`Importer::import_cards`].
    pub async fn import_rulings<I>(&self, records: I) -> Result<RulingImportReport, ImportError>
    where
        I: IntoIterator<Item = Result<RulingRecord, ScryfallError>>,
        I::IntoIter: Send,
    {
        let run = self.start(ImportSource::ScryfallRulings).await?;
        let mut report = RulingImportReport {
            run_id: run,
            inserted: 0,
            unchanged: 0,
            orphaned: 0,
            skipped: SkipLog::default(),
        };
        let outcome = self.ingest_rulings(records.into_iter(), &mut report).await;
        self.conclude(run, report, outcome).await
    }

    async fn ingest_rulings<I>(
        &self,
        records: I,
        report: &mut RulingImportReport,
    ) -> Result<(), StepError>
    where
        I: Iterator<Item = Result<RulingRecord, ScryfallError>> + Send,
    {
        let mut batches = Batches::new(records, self.batch_size);
        loop {
            let batch = batches.next_batch(&mut report.skipped)?;
            if batch.is_empty() {
                return Ok(());
            }
            let mut rows = Vec::with_capacity(batch.len());
            for (position, record) in &batch {
                match ruling_row(*position, record) {
                    Ok(row) => rows.push(row),
                    Err(rejection) => report.skipped.record(*position, rejection),
                }
            }
            if !rows.is_empty() {
                self.write_ruling_batch(&rows, report).await?;
            }
        }
    }

    async fn write_ruling_batch(
        &self,
        rows: &[RulingPayload],
        report: &mut RulingImportReport,
    ) -> Result<(), DbError> {
        let counts = sqlx::query!(
            r#"WITH incoming AS (
                   SELECT seq, oracle_id, source, published_at, comment
                   FROM jsonb_to_recordset($1) AS r(
                       seq bigint, oracle_id uuid, source text, published_at date, comment text)
               ), known AS (
                   SELECT i.* FROM incoming i JOIN cards c ON c.oracle_id = i.oracle_id
               ), inserted AS (
                   INSERT INTO rulings (oracle_id, source, published_at, comment)
                   SELECT oracle_id, source, published_at, comment FROM known ORDER BY seq
                   ON CONFLICT ON CONSTRAINT rulings_natural_key DO NOTHING
                   RETURNING id
               )
               SELECT (SELECT count(*) FROM incoming) AS "received!",
                      (SELECT count(*) FROM known) AS "known!",
                      (SELECT count(*) FROM inserted) AS "inserted!""#,
            Json(rows) as _
        )
        .fetch_one(&self.pool)
        .await?;
        let (received, known, inserted) = (
            counts.received.unsigned_abs(),
            counts.known.unsigned_abs(),
            counts.inserted.unsigned_abs(),
        );
        report.inserted += inserted;
        report.unchanged += known - inserted;
        report.orphaned += received - known;
        Ok(())
    }
}
