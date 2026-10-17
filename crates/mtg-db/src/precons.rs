//! Reading precon decks: listing and loading one with its cards resolved.

use std::collections::HashMap;

use chrono::NaiveDate;
use mtg_import::ids::{OracleId, PrintingId};
use serde::Serialize;
use sqlx::PgPool;
use uuid::Uuid;

use crate::cards::CardRepo;
use crate::error::DbError;
use crate::model::Card;

/// Identifier of a stored precon deck.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, sqlx::Type)]
#[serde(transparent)]
#[sqlx(transparent)]
pub struct PreconDeckId(pub i64);

/// Part of a precon deck an entry belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DeckBoard {
    /// Commander(s) in the command zone.
    Commander,
    /// Main deck.
    Main,
    /// Sideboard.
    Sideboard,
}

impl DeckBoard {
    /// The keyword stored in the `board` column.
    pub const fn keyword(self) -> &'static str {
        match self {
            DeckBoard::Commander => "commander",
            DeckBoard::Main => "main",
            DeckBoard::Sideboard => "sideboard",
        }
    }

    fn from_keyword(keyword: &str) -> Option<Self> {
        [DeckBoard::Commander, DeckBoard::Main, DeckBoard::Sideboard]
            .into_iter()
            .find(|board| board.keyword() == keyword)
    }
}

/// A precon deck without its cards.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreconDeckSummary {
    /// Deck identifier.
    pub id: PreconDeckId,
    /// Code of the set or product the deck came with.
    pub code: String,
    /// Deck name.
    pub name: String,
    /// Deck type, such as `Commander Deck`.
    pub deck_type: Option<String>,
    /// Release date.
    pub release_date: Option<NaiveDate>,
    /// Total number of cards across all boards.
    pub card_count: u64,
    /// Number of those cards that could not be matched to a stored card.
    pub unresolved_count: u64,
}

/// One line of a precon deck.
#[derive(Debug, Clone, PartialEq)]
pub struct PreconEntry {
    /// Order within the deck: commanders, then main deck, then sideboard.
    pub position: u32,
    /// Board the entry belongs to.
    pub board: DeckBoard,
    /// Number of copies.
    pub count: u32,
    /// Card name as the deck file spells it.
    pub card_name: String,
    /// Whether the copies are foil.
    pub is_foil: bool,
    /// Scryfall printing id, if the deck file names one.
    pub printing_id: Option<PrintingId>,
    /// Set code of the printing, if given.
    pub set_code: Option<String>,
    /// Collector number of the printing, if given.
    pub collector_number: Option<String>,
    /// The stored card; `None` when the entry could not be matched.
    pub card: Option<Card>,
}

/// A precon deck with every entry and its card.
#[derive(Debug, Clone, PartialEq)]
pub struct PreconDeckDetail {
    /// Deck header and totals.
    pub summary: PreconDeckSummary,
    /// Entries in deck order.
    pub entries: Vec<PreconEntry>,
}

/// Raw row of the deck listing queries.
struct SummaryRow {
    id: PreconDeckId,
    code: String,
    name: String,
    deck_type: Option<String>,
    release_date: Option<NaiveDate>,
    card_count: i64,
    unresolved_count: i64,
}

impl TryFrom<SummaryRow> for PreconDeckSummary {
    type Error = DbError;

    fn try_from(row: SummaryRow) -> Result<Self, DbError> {
        let total = |column, value: i64| {
            u64::try_from(value).map_err(|_| DbError::Corrupt {
                column,
                value: value.to_string(),
            })
        };
        Ok(Self {
            id: row.id,
            code: row.code,
            name: row.name,
            deck_type: row.deck_type,
            release_date: row.release_date,
            card_count: total("precon_deck_cards.count", row.card_count)?,
            unresolved_count: total("precon_deck_cards.count", row.unresolved_count)?,
        })
    }
}

/// Lists and loads stored precon decks.
#[derive(Debug, Clone)]
pub struct PreconRepo {
    pool: PgPool,
}

impl PreconRepo {
    pub(crate) fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// All decks, newest release first, then by name.
    pub async fn list(&self) -> Result<Vec<PreconDeckSummary>, DbError> {
        let rows = sqlx::query_as!(
            SummaryRow,
            r#"SELECT d.id AS "id: _", d.code, d.name, d.deck_type, d.release_date,
                      COALESCE(SUM(c.count), 0)::bigint AS "card_count!",
                      COALESCE(SUM(c.count) FILTER (WHERE c.oracle_id IS NULL), 0)::bigint
                          AS "unresolved_count!"
               FROM precon_decks d LEFT JOIN precon_deck_cards c ON c.deck_id = d.id
               GROUP BY d.id
               ORDER BY d.release_date DESC NULLS LAST, d.name, d.id"#
        )
        .fetch_all(&self.pool)
        .await?;
        rows.into_iter().map(PreconDeckSummary::try_from).collect()
    }

    /// Loads one deck with every entry matched to its stored card.
    pub async fn get(&self, id: PreconDeckId) -> Result<Option<PreconDeckDetail>, DbError> {
        let row = sqlx::query_as!(
            SummaryRow,
            r#"SELECT d.id AS "id: _", d.code, d.name, d.deck_type, d.release_date,
                      COALESCE(SUM(c.count), 0)::bigint AS "card_count!",
                      COALESCE(SUM(c.count) FILTER (WHERE c.oracle_id IS NULL), 0)::bigint
                          AS "unresolved_count!"
               FROM precon_decks d LEFT JOIN precon_deck_cards c ON c.deck_id = d.id
               WHERE d.id = $1
               GROUP BY d.id"#,
            id as _
        )
        .fetch_optional(&self.pool)
        .await?;
        let Some(row) = row else { return Ok(None) };
        let summary = PreconDeckSummary::try_from(row)?;

        let rows = sqlx::query!(
            "SELECT position, board, card_name, oracle_id, printing_id, set_code,
                    collector_number, count, is_foil
             FROM precon_deck_cards WHERE deck_id = $1 ORDER BY position",
            id as _
        )
        .fetch_all(&self.pool)
        .await?;

        let wanted: Vec<OracleId> = rows
            .iter()
            .filter_map(|row| row.oracle_id)
            .map(|oracle_id: Uuid| OracleId(oracle_id.to_string()))
            .collect();
        let cards: HashMap<OracleId, Card> = CardRepo::new(self.pool.clone())
            .get_many(&wanted)
            .await?
            .into_iter()
            .map(|card| (card.oracle_id.clone(), card))
            .collect();

        let entries = rows
            .into_iter()
            .map(|row| {
                let oracle_id = row.oracle_id.map(|id| OracleId(id.to_string()));
                Ok(PreconEntry {
                    position: unsigned("precon_deck_cards.position", row.position)?,
                    board: DeckBoard::from_keyword(&row.board).ok_or_else(|| DbError::Corrupt {
                        column: "precon_deck_cards.board",
                        value: row.board.clone(),
                    })?,
                    count: unsigned("precon_deck_cards.count", row.count)?,
                    card_name: row.card_name,
                    is_foil: row.is_foil,
                    printing_id: row.printing_id.map(|id| PrintingId(id.to_string())),
                    set_code: row.set_code,
                    collector_number: row.collector_number,
                    // Cloned because one card may appear on several lines (e.g. basic lands).
                    card: oracle_id.and_then(|id| cards.get(&id).cloned()),
                })
            })
            .collect::<Result<Vec<_>, DbError>>()?;
        Ok(Some(PreconDeckDetail { summary, entries }))
    }
}

fn unsigned(column: &'static str, value: i32) -> Result<u32, DbError> {
    u32::try_from(value).map_err(|_| DbError::Corrupt {
        column,
        value: value.to_string(),
    })
}
