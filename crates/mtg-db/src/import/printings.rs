//! Idempotent batched upsert of printings: the statement the card import shares and the import of an all-printings file.

use std::collections::BTreeMap;

use mtg_import::scryfall::{CardRecord, ScryfallError};
use serde::Serialize;
use sqlx::PgConnection;
use sqlx::types::Json;

use super::batches::{Batches, SkipLog};
use super::card_payload::{PrintingPayload, split_printing};
use super::cards::UpsertCounts;
use super::{ImportError, Importer, StepError};
use crate::error::DbError;
use crate::runs::{ImportSource, RunId};

/// What one statement did with a batch of printings.
#[derive(Debug, Clone, Copy)]
pub(super) struct BatchCounts {
    /// Printings offered.
    pub received: u64,
    /// Offered printings whose card is stored.
    pub known: u64,
    /// Rows that were newly inserted or changed.
    pub written: u64,
    /// Rows that were newly inserted.
    pub inserted: u64,
}

/// Upserts the printings whose card is stored; the others are left out and counted in `received - known`.
pub(super) async fn upsert_printings(
    conn: &mut PgConnection,
    printings: &[PrintingPayload],
) -> Result<BatchCounts, sqlx::Error> {
    // `xmax = 0` tells an inserted row from a conflict-updated one; the WHERE clause skips no-op updates.
    let counts = sqlx::query!(
        r#"WITH incoming AS (
               SELECT printing_id, oracle_id, set_code, collector_number, rarity, released_at,
                      lang, digital, images, flavor_name, printed_name
               FROM jsonb_to_recordset($1) AS r(
                   printing_id uuid, oracle_id uuid, set_code text, collector_number text,
                   rarity text, released_at date, lang text, digital boolean, images jsonb,
                   flavor_name text, printed_name text)
           ), known AS (
               SELECT i.* FROM incoming i JOIN cards c ON c.oracle_id = i.oracle_id
           ), written AS (
               INSERT INTO printings AS p (
                   printing_id, oracle_id, set_code, collector_number, rarity, released_at, lang,
                   digital, images, flavor_name, printed_name)
               SELECT printing_id, oracle_id, set_code, collector_number, rarity, released_at,
                      lang, digital, images, flavor_name, printed_name
               FROM known ORDER BY printing_id
               ON CONFLICT (printing_id) DO UPDATE SET
                   oracle_id = EXCLUDED.oracle_id,
                   set_code = EXCLUDED.set_code,
                   collector_number = EXCLUDED.collector_number,
                   rarity = EXCLUDED.rarity,
                   released_at = EXCLUDED.released_at,
                   lang = EXCLUDED.lang,
                   digital = EXCLUDED.digital,
                   images = EXCLUDED.images,
                   flavor_name = EXCLUDED.flavor_name,
                   printed_name = EXCLUDED.printed_name,
                   updated_at = now()
               WHERE (p.oracle_id, p.set_code, p.collector_number, p.rarity, p.released_at,
                      p.lang, p.digital, p.images, p.flavor_name, p.printed_name)
                   IS DISTINCT FROM
                     (EXCLUDED.oracle_id, EXCLUDED.set_code, EXCLUDED.collector_number,
                      EXCLUDED.rarity, EXCLUDED.released_at, EXCLUDED.lang, EXCLUDED.digital,
                      EXCLUDED.images, EXCLUDED.flavor_name, EXCLUDED.printed_name)
               RETURNING (xmax = 0) AS inserted
           )
           SELECT (SELECT count(*) FROM incoming) AS "received!",
                  (SELECT count(*) FROM known) AS "known!",
                  (SELECT count(*) FROM written) AS "written!",
                  (SELECT count(*) FROM written WHERE inserted) AS "inserted!""#,
        Json(printings) as _
    )
    .fetch_one(conn)
    .await?;
    Ok(BatchCounts {
        received: counts.received.unsigned_abs(),
        known: counts.known.unsigned_abs(),
        written: counts.written.unsigned_abs(),
        inserted: counts.inserted.unsigned_abs(),
    })
}

/// Outcome of an all-printings import; counts are per batch.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PrintingImportReport {
    /// The run that recorded this import.
    pub run_id: RunId,
    /// Changes to printings.
    pub printings: UpsertCounts,
    /// Printings whose card is not in the database, so they were not stored.
    pub orphaned: u64,
    /// Records that were not imported.
    pub skipped: SkipLog,
}

impl Importer {
    /// Upserts the printings of `records` in batches, recording the run; cards are never written.
    ///
    /// Only the printing-level data of a record is read (ids, set, number, language, art, flavor
    /// and printed names). A file with every printing of a card therefore cannot change the
    /// oracle-level data the Oracle Cards import owns, and a record that names a card of its own
    /// (a reversible card, whose faces are other cards) adds a printing to the stored card.
    /// Printings of cards that are not stored are counted as orphaned, so import cards first.
    /// Failure and iteration behave as in [`Importer::import_cards`].
    pub async fn import_printings<I>(&self, records: I) -> Result<PrintingImportReport, ImportError>
    where
        I: IntoIterator<Item = Result<CardRecord, ScryfallError>>,
        I::IntoIter: Send,
    {
        let run = self.start(ImportSource::ScryfallPrintings).await?;
        let mut report = PrintingImportReport {
            run_id: run,
            printings: UpsertCounts::default(),
            orphaned: 0,
            skipped: SkipLog::default(),
        };
        let outcome = self
            .ingest_printings(records.into_iter(), &mut report)
            .await;
        self.conclude(run, report, outcome).await
    }

    async fn ingest_printings<I>(
        &self,
        records: I,
        report: &mut PrintingImportReport,
    ) -> Result<(), StepError>
    where
        I: Iterator<Item = Result<CardRecord, ScryfallError>> + Send,
    {
        let mut batches = Batches::new(records, self.batch_size);
        loop {
            let batch = batches.next_batch(&mut report.skipped)?;
            if batch.is_empty() {
                return Ok(());
            }
            // Keyed by id: repeats within the batch collapse (last wins) and rows lock in one order.
            let mut printings = BTreeMap::new();
            for (position, record) in &batch {
                match split_printing(record) {
                    Ok(printing) => {
                        printings.insert(printing.printing_id, printing);
                    }
                    Err(rejection) => report.skipped.record(*position, rejection),
                }
            }
            if !printings.is_empty() {
                let printings: Vec<PrintingPayload> = printings.into_values().collect();
                self.write_printing_batch(&printings, report).await?;
            }
        }
    }

    async fn write_printing_batch(
        &self,
        printings: &[PrintingPayload],
        report: &mut PrintingImportReport,
    ) -> Result<(), DbError> {
        let mut conn = self.pool.acquire().await?;
        let counts = upsert_printings(&mut conn, printings).await?;
        report.printings.add_counts(counts);
        report.orphaned += counts.received - counts.known;
        Ok(())
    }
}
