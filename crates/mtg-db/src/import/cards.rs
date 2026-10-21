//! Idempotent batched upsert of cards and printings from a Scryfall record stream.

use std::collections::BTreeMap;

use mtg_import::scryfall::{CardRecord, ScryfallError};
use serde::Serialize;
use sqlx::types::Json;
use sqlx::{PgConnection, Postgres, Transaction};

use super::batches::{Batches, SkipLog};
use super::card_payload::{CardPayload, PrintingPayload, split_record};
use super::printings::{BatchCounts, upsert_printings};
use super::{ImportError, Importer, StepError};
use crate::error::DbError;
use crate::runs::{ImportSource, RunId};

/// How the rows of one table changed.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct UpsertCounts {
    /// Rows that did not exist before.
    pub inserted: u64,
    /// Existing rows whose data changed.
    pub updated: u64,
    /// Existing rows that already held exactly this data.
    pub unchanged: u64,
}

impl UpsertCounts {
    /// Adds a batch where `inserted_flags` has one entry per row that was written and `submitted` rows were offered.
    fn add_batch(&mut self, inserted_flags: &[bool], submitted: usize) {
        let inserted = inserted_flags.iter().filter(|&&inserted| inserted).count() as u64;
        let written = inserted_flags.len() as u64;
        self.inserted += inserted;
        self.updated += written - inserted;
        self.unchanged += submitted as u64 - written;
    }

    /// Adds a batch of printings; the offered rows whose card is stored and were not written are unchanged.
    pub(super) fn add_counts(&mut self, counts: BatchCounts) {
        self.inserted += counts.inserted;
        self.updated += counts.written - counts.inserted;
        self.unchanged += counts.known - counts.written;
    }
}

/// Outcome of a card import; counts are per batch, so a card spread over several batches is counted in each.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CardImportReport {
    /// The run that recorded this import.
    pub run_id: RunId,
    /// Changes to oracle cards.
    pub cards: UpsertCounts,
    /// Changes to printings.
    pub printings: UpsertCounts,
    /// Records that were not imported.
    pub skipped: SkipLog,
}

impl CardImportReport {
    fn new(run_id: RunId) -> Self {
        Self {
            run_id,
            cards: UpsertCounts::default(),
            printings: UpsertCounts::default(),
            skipped: SkipLog::default(),
        }
    }
}

impl Importer {
    /// Upserts cards and printings from `records` in batches, recording the run.
    ///
    /// Unusable records are counted and skipped; a fatal stream error or a database error aborts
    /// the import, marks the run failed and leaves earlier batches committed. Re-importing the
    /// same data changes nothing. The iterator is advanced on the calling task, so slow sources
    /// should be read ahead on a blocking thread.
    pub async fn import_cards<I>(&self, records: I) -> Result<CardImportReport, ImportError>
    where
        I: IntoIterator<Item = Result<CardRecord, ScryfallError>>,
        I::IntoIter: Send,
    {
        let run = self.start(ImportSource::ScryfallCards).await?;
        let mut report = CardImportReport::new(run);
        let outcome = self.ingest_cards(records.into_iter(), &mut report).await;
        self.conclude(run, report, outcome).await
    }

    async fn ingest_cards<I>(
        &self,
        records: I,
        report: &mut CardImportReport,
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
            self.write_card_batch(batch, report).await?;
        }
    }

    async fn write_card_batch(
        &self,
        batch: Vec<(u64, CardRecord)>,
        report: &mut CardImportReport,
    ) -> Result<(), DbError> {
        // Keyed maps drop repeats within the batch (last wins) and fix the lock order across runs.
        let mut cards = BTreeMap::new();
        let mut printings = BTreeMap::new();
        for (position, record) in batch {
            match split_record(&record) {
                Ok((card, printing)) => {
                    cards.insert(card.oracle_id, card);
                    printings.insert(printing.printing_id, printing);
                }
                Err(rejection) => report.skipped.record(position, rejection),
            }
        }
        if cards.is_empty() {
            return Ok(());
        }
        let cards: Vec<CardPayload> = cards.into_values().collect();
        let printings: Vec<PrintingPayload> = printings.into_values().collect();

        let mut tx: Transaction<'_, Postgres> = self.pool.begin().await?;
        let card_flags = upsert_cards(&mut tx, &cards).await?;
        let printing_counts = upsert_printings(&mut tx, &printings).await?;
        tx.commit().await?;

        report.cards.add_batch(&card_flags, cards.len());
        report.printings.add_counts(printing_counts);
        Ok(())
    }
}

/// Upserts oracle cards, leaving `impl_status` alone; returns one flag per written row, true if newly inserted.
async fn upsert_cards(
    conn: &mut PgConnection,
    cards: &[CardPayload],
) -> Result<Vec<bool>, sqlx::Error> {
    // `xmax = 0` is the usual way to tell an inserted row from a conflict-updated one.
    // The WHERE clause skips no-op updates, so unchanged rows are neither rewritten nor returned.
    sqlx::query_scalar!(
        r#"INSERT INTO cards AS c (
               oracle_id, name, name_normalized, front_name_normalized, layout, is_deck_card,
               mana_cost, mana_value, type_line, oracle_text, keywords, colors, color_identity,
               produced_mana, produces_colorless, legalities, faces, melds_into)
           SELECT oracle_id, name, name_normalized, front_name_normalized, layout, is_deck_card,
               mana_cost, mana_value, type_line, oracle_text, keywords, colors, color_identity,
               produced_mana, produces_colorless, legalities, faces, melds_into
           FROM jsonb_to_recordset($1) AS r(
               oracle_id uuid, name text, name_normalized text, front_name_normalized text,
               layout text, is_deck_card boolean, mana_cost text, mana_value double precision,
               type_line text, oracle_text text, keywords text[], colors smallint,
               color_identity smallint, produced_mana smallint, produces_colorless boolean,
               legalities jsonb, faces jsonb, melds_into text)
           ORDER BY oracle_id
           ON CONFLICT (oracle_id) DO UPDATE SET
               name = EXCLUDED.name,
               name_normalized = EXCLUDED.name_normalized,
               front_name_normalized = EXCLUDED.front_name_normalized,
               layout = EXCLUDED.layout,
               is_deck_card = EXCLUDED.is_deck_card,
               mana_cost = EXCLUDED.mana_cost,
               mana_value = EXCLUDED.mana_value,
               type_line = EXCLUDED.type_line,
               oracle_text = EXCLUDED.oracle_text,
               keywords = EXCLUDED.keywords,
               colors = EXCLUDED.colors,
               color_identity = EXCLUDED.color_identity,
               produced_mana = EXCLUDED.produced_mana,
               produces_colorless = EXCLUDED.produces_colorless,
               legalities = EXCLUDED.legalities,
               faces = EXCLUDED.faces,
               melds_into = EXCLUDED.melds_into,
               updated_at = now()
           WHERE (c.name, c.name_normalized, c.front_name_normalized, c.layout, c.is_deck_card,
                  c.mana_cost, c.mana_value, c.type_line, c.oracle_text, c.keywords, c.colors,
                  c.color_identity, c.produced_mana, c.produces_colorless, c.legalities, c.faces,
                  c.melds_into)
               IS DISTINCT FROM
                 (EXCLUDED.name, EXCLUDED.name_normalized, EXCLUDED.front_name_normalized,
                  EXCLUDED.layout, EXCLUDED.is_deck_card, EXCLUDED.mana_cost, EXCLUDED.mana_value,
                  EXCLUDED.type_line, EXCLUDED.oracle_text, EXCLUDED.keywords, EXCLUDED.colors,
                  EXCLUDED.color_identity, EXCLUDED.produced_mana, EXCLUDED.produces_colorless,
                  EXCLUDED.legalities, EXCLUDED.faces, EXCLUDED.melds_into)
           RETURNING (xmax = 0) AS "inserted!""#,
        Json(cards) as _
    )
    .fetch_all(conn)
    .await
}

#[cfg(test)]
mod tests {
    //! Goal: batch tallies split written rows into inserted and updated and attribute every unwritten row to "unchanged".
    use rstest::rstest;

    use super::*;

    #[rstest]
    #[case(&[true, true, true], 3, (3, 0, 0))]
    #[case(&[false, false], 2, (0, 2, 0))]
    #[case(&[], 4, (0, 0, 4))]
    #[case(&[true, false], 5, (1, 1, 3))]
    fn tallies_partition_the_submitted_rows(
        #[case] flags: &[bool],
        #[case] submitted: usize,
        #[case] expected: (u64, u64, u64),
    ) {
        let mut counts = UpsertCounts::default();
        counts.add_batch(flags, submitted);
        assert_eq!(
            (counts.inserted, counts.updated, counts.unchanged),
            expected
        );
        assert_eq!(
            counts.inserted + counts.updated + counts.unchanged,
            submitted as u64
        );
    }
}
