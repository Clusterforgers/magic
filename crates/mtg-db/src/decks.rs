//! Stored decks: a deck per row with its lines, owned by an account of the server, and the last
//! check the server ran on it. Every query names the owner, so one account never reads, replaces or
//! deletes another's deck, and the cap on decks per account is enforced under a lock.

use std::fmt;

use mtg_import::ids::{OracleId, PrintingId};
use serde_json::Value;
use sqlx::types::Json;
use sqlx::{PgConnection, PgPool, Postgres, QueryBuilder};
use thiserror::Error;
use uuid::Uuid;

use crate::error::DbError;

/// First half of the advisory-lock key that serialises the creation of decks of one owner ("MTGD").
const OWNER_LOCK_CLASS: i32 = 0x4d54_4744;

/// Identifier of a stored deck.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DeckId(pub Uuid);

impl fmt::Display for DeckId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.0, f)
    }
}

/// A part of a stored deck.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum DeckZone {
    /// The main deck.
    Main,
    /// The sideboard.
    Sideboard,
    /// The command zone.
    Commander,
    /// The companion.
    Companion,
}

impl DeckZone {
    /// The keyword stored in the `zone` column.
    pub const fn keyword(self) -> &'static str {
        match self {
            DeckZone::Main => "main",
            DeckZone::Sideboard => "sideboard",
            DeckZone::Commander => "commander",
            DeckZone::Companion => "companion",
        }
    }

    /// Reads a stored keyword.
    pub fn from_keyword(keyword: &str) -> Option<DeckZone> {
        [
            DeckZone::Main,
            DeckZone::Sideboard,
            DeckZone::Commander,
            DeckZone::Companion,
        ]
        .into_iter()
        .find(|zone| zone.keyword() == keyword)
    }
}

/// Copies of one card in one zone, with the printing the owner chose, if any.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeckLine {
    /// The zone.
    pub zone: DeckZone,
    /// The card.
    pub oracle_id: OracleId,
    /// The chosen printing, which must be one of the card.
    pub printing_id: Option<PrintingId>,
    /// How many copies, 1 to 999.
    pub count: u32,
}

/// What a save writes: a deck as the owner sent it, and the check run on it.
#[derive(Debug, Clone, PartialEq)]
pub struct DeckDraft {
    /// The deck's name, 1 to 200 characters.
    pub name: String,
    /// The format key (`commander`, `modern`, `freeform`, ...).
    pub format: String,
    /// The owner's notes, up to 20 000 characters.
    pub notes: String,
    /// The lines in the order the owner keeps them.
    pub lines: Vec<DeckLine>,
    /// The check run on this content: `{deck_hash, coverage_hash, checked_at, summary}`.
    pub check: Option<Value>,
}

/// A deck as stored.
#[derive(Debug, Clone, PartialEq)]
pub struct StoredDeck {
    /// Identifier of the deck.
    pub id: DeckId,
    /// The deck's name.
    pub name: String,
    /// The format key.
    pub format: String,
    /// The owner's notes.
    pub notes: String,
    /// The lines in the owner's order.
    pub lines: Vec<DeckLine>,
    /// The check stored with the deck; `None` for a deck saved without one.
    pub check: Option<Value>,
    /// When the deck was created, as `YYYY-MM-DDTHH:MM:SSZ` (UTC).
    pub created_at: String,
    /// When the deck was last saved, in the same form.
    pub updated_at: String,
}

/// Why a deck could not be created.
#[derive(Debug, Error)]
pub enum CreateDeckError {
    /// The owner already holds as many decks as allowed.
    #[error("the owner already holds {max} decks")]
    LimitReached {
        /// The cap that was met.
        max: u32,
    },
    /// The database failed.
    #[error(transparent)]
    Database(#[from] DbError),
}

impl From<sqlx::Error> for CreateDeckError {
    fn from(error: sqlx::Error) -> Self {
        CreateDeckError::Database(error.into())
    }
}

/// Reads and writes stored decks.
#[derive(Debug, Clone)]
pub struct DeckRepo {
    pool: PgPool,
}

/// A `decks` row as every deck query selects it.
struct DeckRow {
    id: Uuid,
    name: String,
    format: String,
    notes: String,
    last_check: Option<Json<Value>>,
    created_at: String,
    updated_at: String,
}

/// A `deck_cards` row.
struct LineRow {
    deck_id: Uuid,
    zone: String,
    oracle_id: Uuid,
    printing_id: Option<Uuid>,
    count: i32,
}

fn line_of(row: &LineRow) -> Result<DeckLine, DbError> {
    let zone = DeckZone::from_keyword(&row.zone).ok_or_else(|| DbError::Corrupt {
        column: "deck_cards.zone",
        value: row.zone.clone(),
    })?;
    let count = u32::try_from(row.count).map_err(|_| DbError::Corrupt {
        column: "deck_cards.count",
        value: row.count.to_string(),
    })?;
    Ok(DeckLine {
        zone,
        oracle_id: OracleId(row.oracle_id.to_string()),
        printing_id: row.printing_id.map(|id| PrintingId(id.to_string())),
        count,
    })
}

/// The decks of `rows` with their lines, in the order of `rows`.
async fn with_lines(
    conn: &mut PgConnection,
    rows: Vec<DeckRow>,
) -> Result<Vec<StoredDeck>, DbError> {
    let ids: Vec<Uuid> = rows.iter().map(|row| row.id).collect();
    let line_rows = sqlx::query_as!(
        LineRow,
        r#"SELECT deck_id, zone, oracle_id, printing_id, count
           FROM deck_cards WHERE deck_id = ANY($1) ORDER BY deck_id, position"#,
        &ids
    )
    .fetch_all(&mut *conn)
    .await?;
    let mut by_deck: std::collections::HashMap<Uuid, Vec<DeckLine>> =
        std::collections::HashMap::new();
    for row in line_rows {
        let deck = row.deck_id;
        by_deck.entry(deck).or_default().push(line_of(&row)?);
    }
    Ok(rows
        .into_iter()
        .map(|row| StoredDeck {
            lines: by_deck.remove(&row.id).unwrap_or_default(),
            id: DeckId(row.id),
            name: row.name,
            format: row.format,
            notes: row.notes,
            check: row.last_check.map(|check| check.0),
            created_at: row.created_at,
            updated_at: row.updated_at,
        })
        .collect())
}

async fn load(
    conn: &mut PgConnection,
    owner: Uuid,
    id: Uuid,
) -> Result<Option<StoredDeck>, DbError> {
    let row = sqlx::query_as!(
        DeckRow,
        r#"SELECT id, name, format, notes, last_check AS "last_check: Json<Value>",
                  to_char(created_at AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS"Z"') AS "created_at!",
                  to_char(updated_at AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS"Z"') AS "updated_at!"
           FROM decks WHERE owner = $1 AND id = $2"#,
        owner,
        id
    )
    .fetch_optional(&mut *conn)
    .await?;
    match row {
        Some(row) => Ok(with_lines(conn, vec![row]).await?.pop()),
        None => Ok(None),
    }
}

/// Writes the lines of a deck, in order.
async fn write_lines(
    conn: &mut PgConnection,
    deck: Uuid,
    lines: &[DeckLine],
) -> Result<(), DbError> {
    if lines.is_empty() {
        return Ok(());
    }
    let mut ids = Vec::with_capacity(lines.len());
    for line in lines {
        let oracle = Uuid::parse_str(line.oracle_id.as_str()).map_err(|_| DbError::Corrupt {
            column: "deck_cards.oracle_id",
            value: line.oracle_id.to_string(),
        })?;
        let printing = line
            .printing_id
            .as_ref()
            .map(|id| {
                Uuid::parse_str(id.as_str()).map_err(|_| DbError::Corrupt {
                    column: "deck_cards.printing_id",
                    value: id.to_string(),
                })
            })
            .transpose()?;
        ids.push((oracle, printing));
    }
    let mut insert = QueryBuilder::<Postgres>::new(
        "INSERT INTO deck_cards (deck_id, position, zone, oracle_id, printing_id, count) ",
    );
    insert.push_values(
        lines.iter().zip(ids).enumerate(),
        |mut row, (position, (line, (oracle, printing)))| {
            row.push_bind(deck)
                .push_bind(i32::try_from(position).unwrap_or(i32::MAX))
                .push_bind(line.zone.keyword())
                .push_bind(oracle)
                .push_bind(printing)
                .push_bind(i32::try_from(line.count).unwrap_or(i32::MAX));
        },
    );
    insert.build().execute(&mut *conn).await?;
    Ok(())
}

impl DeckRepo {
    pub(crate) fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Stores a new deck for `owner`, unless the owner already holds `max_decks`. The count and the
    /// insert run under a lock of the owner, so racing creations cannot exceed the cap.
    pub async fn create(
        &self,
        owner: Uuid,
        draft: &DeckDraft,
        max_decks: u32,
    ) -> Result<StoredDeck, CreateDeckError> {
        let mut tx = self.pool.begin().await?;
        sqlx::query!(
            "SELECT pg_advisory_xact_lock($1, hashtext($2::text))",
            OWNER_LOCK_CLASS,
            owner.to_string()
        )
        .execute(&mut *tx)
        .await?;
        let held = sqlx::query_scalar!(
            r#"SELECT count(*) AS "held!" FROM decks WHERE owner = $1"#,
            owner
        )
        .fetch_one(&mut *tx)
        .await?;
        if held >= i64::from(max_decks) {
            return Err(CreateDeckError::LimitReached { max: max_decks });
        }
        let id = sqlx::query_scalar!(
            r#"INSERT INTO decks (owner, name, format, notes, last_check)
               VALUES ($1, $2, $3, $4, $5) RETURNING id"#,
            owner,
            draft.name,
            draft.format,
            draft.notes,
            draft.check.as_ref().map(Json) as Option<Json<&Value>>
        )
        .fetch_one(&mut *tx)
        .await?;
        write_lines(&mut tx, id, &draft.lines).await?;
        let stored = load(&mut tx, owner, id)
            .await?
            .ok_or_else(|| DbError::Corrupt {
                column: "decks.id",
                value: id.to_string(),
            })?;
        tx.commit().await?;
        Ok(stored)
    }

    /// The deck `id` of `owner`; another owner's deck and an unknown id look alike.
    pub async fn get(&self, owner: Uuid, id: DeckId) -> Result<Option<StoredDeck>, DbError> {
        let mut conn = self.pool.acquire().await?;
        load(&mut conn, owner, id.0).await
    }

    /// Every deck of `owner`, the most recently saved first.
    pub async fn list(&self, owner: Uuid) -> Result<Vec<StoredDeck>, DbError> {
        let mut conn = self.pool.acquire().await?;
        let rows = sqlx::query_as!(
            DeckRow,
            r#"SELECT id, name, format, notes, last_check AS "last_check: Json<Value>",
                      to_char(created_at AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS"Z"') AS "created_at!",
                      to_char(updated_at AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS"Z"') AS "updated_at!"
               FROM decks WHERE owner = $1 ORDER BY updated_at DESC, id"#,
            owner
        )
        .fetch_all(&mut *conn)
        .await?;
        with_lines(&mut conn, rows).await
    }

    /// Replaces the name, format, notes, lines and check of the deck `id` of `owner` in one
    /// transaction; `None` if the owner holds no such deck.
    pub async fn replace(
        &self,
        owner: Uuid,
        id: DeckId,
        draft: &DeckDraft,
    ) -> Result<Option<StoredDeck>, DbError> {
        let mut tx = self.pool.begin().await?;
        let updated = sqlx::query_scalar!(
            r#"UPDATE decks SET name = $3, format = $4, notes = $5, last_check = $6, updated_at = now()
               WHERE owner = $1 AND id = $2 RETURNING id"#,
            owner,
            id.0,
            draft.name,
            draft.format,
            draft.notes,
            draft.check.as_ref().map(Json) as Option<Json<&Value>>
        )
        .fetch_optional(&mut *tx)
        .await?;
        let Some(deck) = updated else {
            return Ok(None);
        };
        sqlx::query!("DELETE FROM deck_cards WHERE deck_id = $1", deck)
            .execute(&mut *tx)
            .await?;
        write_lines(&mut tx, deck, &draft.lines).await?;
        let stored = load(&mut tx, owner, deck).await?;
        tx.commit().await?;
        Ok(stored)
    }

    /// Deletes the deck `id` of `owner`; false if the owner holds no such deck.
    pub async fn delete(&self, owner: Uuid, id: DeckId) -> Result<bool, DbError> {
        let deleted = sqlx::query!(
            "DELETE FROM decks WHERE owner = $1 AND id = $2",
            owner,
            id.0
        )
        .execute(&self.pool)
        .await?;
        Ok(deleted.rows_affected() > 0)
    }

    /// How many decks `owner` holds.
    pub async fn count(&self, owner: Uuid) -> Result<u64, DbError> {
        let held = sqlx::query_scalar!(
            r#"SELECT count(*) AS "held!" FROM decks WHERE owner = $1"#,
            owner
        )
        .fetch_one(&self.pool)
        .await?;
        Ok(held.unsigned_abs())
    }
}

#[cfg(test)]
mod tests {
    //! Goal: zone keywords round-trip so a stored line always reads back in the zone it was written to, and an unknown keyword is no zone.
    use rstest::rstest;

    use super::*;

    #[rstest]
    #[case(DeckZone::Main, "main")]
    #[case(DeckZone::Sideboard, "sideboard")]
    #[case(DeckZone::Commander, "commander")]
    #[case(DeckZone::Companion, "companion")]
    fn zones_round_trip_through_their_keyword(#[case] zone: DeckZone, #[case] keyword: &str) {
        assert_eq!(zone.keyword(), keyword);
        assert_eq!(DeckZone::from_keyword(keyword), Some(zone));
    }

    #[rstest]
    #[case("")]
    #[case("Main")]
    #[case("maybeboard")]
    fn other_keywords_are_no_zone(#[case] keyword: &str) {
        assert_eq!(DeckZone::from_keyword(keyword), None);
    }
}
