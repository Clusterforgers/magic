//! Ingest of MTGJSON precon decks, resolving each card against stored cards.

use mtg_import::mtgjson::{MtgjsonError, PreconCard, PreconDeck};
use serde::Serialize;
use sqlx::types::Json;
use sqlx::{PgConnection, Postgres, Transaction};
use uuid::Uuid;

use super::batches::SkipLog;
use super::card_payload::{Rejection, iso_date};
use super::precon_resolve::{CardLookups, UnresolvedReason};
use super::{ImportError, Importer, StepError};
use crate::error::DbError;
use crate::precons::{DeckBoard, PreconDeckId};
use crate::runs::{ImportSource, RunId};

/// A deck entry that could not be matched to a stored card; it is stored without a card.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct UnresolvedCard {
    /// Card name as the deck file spells it.
    pub name: String,
    /// Board the entry is on.
    pub board: DeckBoard,
    /// Number of copies.
    pub count: u32,
    /// Why no card was chosen.
    pub reason: UnresolvedReason,
}

/// Result of ingesting one deck.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DeckIngest {
    /// Stored deck identifier.
    pub deck_id: PreconDeckId,
    /// Deck code.
    pub code: String,
    /// Deck name.
    pub name: String,
    /// Number of entries stored (one per deck-file line).
    pub entries: usize,
    /// Entries that could not be matched to a card.
    pub unresolved: Vec<UnresolvedCard>,
}

/// Outcome of a precon import.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PreconImportReport {
    /// The run that recorded this import.
    pub run_id: RunId,
    /// Decks that were stored, in input order.
    pub decks: Vec<DeckIngest>,
    /// Deck files that were not imported.
    pub skipped: SkipLog,
}

/// A `precon_deck_cards` row in the shape of the insert statement's `jsonb_to_recordset` columns.
#[derive(Debug, Serialize)]
struct EntryPayload {
    position: i32,
    board: &'static str,
    card_name: String,
    oracle_id: Option<Uuid>,
    printing_id: Option<Uuid>,
    set_code: Option<String>,
    collector_number: Option<String>,
    count: i32,
    is_foil: bool,
}

impl Importer {
    /// Stores precon decks, replacing the cards of decks that already exist, and records the run.
    ///
    /// Cards are matched by oracle id, then printing id, then name. Entries that match nothing
    /// (or several cards) are stored without a card and listed in the report. Deck files that
    /// failed to parse, or hold unusable data, are counted and skipped.
    pub async fn import_precons<I>(&self, decks: I) -> Result<PreconImportReport, ImportError>
    where
        I: IntoIterator<Item = Result<PreconDeck, MtgjsonError>>,
        I::IntoIter: Send,
    {
        let run = self.start(ImportSource::MtgjsonDecks).await?;
        let mut report = PreconImportReport {
            run_id: run,
            decks: Vec::new(),
            skipped: SkipLog::default(),
        };
        let outcome = self.ingest_decks(decks.into_iter(), &mut report).await;
        self.conclude(run, report, outcome).await
    }

    async fn ingest_decks<I>(
        &self,
        decks: I,
        report: &mut PreconImportReport,
    ) -> Result<(), StepError>
    where
        I: Iterator<Item = Result<PreconDeck, MtgjsonError>> + Send,
    {
        for (position, deck) in (0u64..).zip(decks) {
            let deck = match deck {
                Ok(deck) => deck,
                Err(error) => {
                    report.skipped.record(position, error);
                    continue;
                }
            };
            match self.write_deck(&deck).await? {
                Ok(ingest) => report.decks.push(ingest),
                Err(rejection) => report
                    .skipped
                    .record(position, format!("{}: {rejection}", deck.name)),
            }
        }
        Ok(())
    }

    /// Stores one deck in its own transaction; unusable data is a rejection, not an error.
    async fn write_deck(
        &self,
        deck: &PreconDeck,
    ) -> Result<Result<DeckIngest, Rejection>, DbError> {
        let release_date = match deck.release_date.as_deref().map(iso_date).transpose() {
            Ok(date) => date,
            Err(rejection) => return Ok(Err(rejection)),
        };
        let boards = [
            (DeckBoard::Commander, &deck.commander),
            (DeckBoard::Main, &deck.main_board),
            (DeckBoard::Sideboard, &deck.side_board),
        ];
        let lines: Vec<(DeckBoard, &PreconCard)> = boards
            .into_iter()
            .flat_map(|(board, cards)| cards.iter().map(move |card| (board, card)))
            .collect();

        let Some(counts) = lines
            .iter()
            .map(|(_, card)| i32::try_from(card.count).ok())
            .collect::<Option<Vec<i32>>>()
        else {
            return Ok(Err(Rejection::CountTooLarge));
        };

        let mut tx: Transaction<'_, Postgres> = self.pool.begin().await?;
        let cards: Vec<&PreconCard> = lines.iter().map(|(_, card)| *card).collect();
        let lookups = CardLookups::load(&mut tx, &cards).await?;

        let mut rows = Vec::with_capacity(lines.len());
        let mut unresolved = Vec::new();
        for (position, ((board, card), count)) in (0i32..).zip(lines.iter().zip(counts)) {
            let oracle_id = match lookups.resolve(card) {
                Ok(id) => Some(id),
                Err(reason) => {
                    unresolved.push(UnresolvedCard {
                        name: card.name.clone(),
                        board: *board,
                        count: card.count,
                        reason,
                    });
                    None
                }
            };
            rows.push(EntryPayload {
                position,
                board: board.keyword(),
                card_name: card.name.clone(),
                oracle_id,
                printing_id: card
                    .scryfall_id
                    .as_ref()
                    .and_then(|id| Uuid::parse_str(id.as_str()).ok()),
                set_code: card.set_code.clone(),
                collector_number: card.number.clone(),
                count,
                is_foil: card.is_foil,
            });
        }

        let deck_id = upsert_deck(&mut tx, deck, release_date.as_deref()).await?;
        replace_entries(&mut tx, deck_id, &rows).await?;
        tx.commit().await?;
        Ok(Ok(DeckIngest {
            deck_id,
            code: deck.code.clone(),
            name: deck.name.clone(),
            entries: rows.len(),
            unresolved,
        }))
    }
}

async fn upsert_deck(
    conn: &mut PgConnection,
    deck: &PreconDeck,
    release_date: Option<&str>,
) -> Result<PreconDeckId, sqlx::Error> {
    sqlx::query_scalar!(
        r#"INSERT INTO precon_decks (code, name, deck_type, release_date)
           VALUES ($1, $2, $3, $4::text::date)
           ON CONFLICT ON CONSTRAINT precon_decks_natural_key DO UPDATE SET
               deck_type = EXCLUDED.deck_type, release_date = EXCLUDED.release_date
           RETURNING id AS "id: PreconDeckId""#,
        deck.code,
        deck.name,
        deck.deck_type,
        release_date
    )
    .fetch_one(conn)
    .await
}

/// Replaces all entries of a deck, so a changed deck file never leaves stale lines behind.
async fn replace_entries(
    conn: &mut PgConnection,
    deck_id: PreconDeckId,
    rows: &[EntryPayload],
) -> Result<(), sqlx::Error> {
    sqlx::query!(
        "DELETE FROM precon_deck_cards WHERE deck_id = $1",
        deck_id as _
    )
    .execute(&mut *conn)
    .await?;
    sqlx::query!(
        "INSERT INTO precon_deck_cards (deck_id, position, board, card_name, oracle_id, printing_id,
                                        set_code, collector_number, count, is_foil)
         SELECT $1, position, board, card_name, oracle_id, printing_id, set_code, collector_number,
                count, is_foil
         FROM jsonb_to_recordset($2) AS r(
             position integer, board text, card_name text, oracle_id uuid, printing_id uuid,
             set_code text, collector_number text, count integer, is_foil boolean)",
        deck_id as _,
        Json(rows) as _
    )
    .execute(conn)
    .await?;
    Ok(())
}
