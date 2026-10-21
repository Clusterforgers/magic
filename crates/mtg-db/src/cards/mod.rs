//! Card queries: lookup by id or name, fuzzy suggestions, filtered search and the name index.

mod lookup;
mod printings;
mod search;

use chrono::{DateTime, Utc};
use mtg_import::ids::OracleId;
use mtg_import::resolve::CardIndex;
use sqlx::PgPool;
use uuid::Uuid;

pub use lookup::{MAX_SUGGESTIONS, NameSuggestion};
pub use search::{
    CardPage, CardSearch, ColorFilter, ColorMatch, DEFAULT_PAGE_SIZE, MAX_PAGE_SIZE, Page, SortKey,
};

use crate::error::DbError;
use crate::model::{Card, CardDetail, Printing, Ruling};
use crate::rows::{CardRow, PrintingRow, RulingRow};

/// What the stored cards and printings looked like when [`CardRepo::index_version`] read them.
///
/// Two equal versions mean no card or printing was added, removed or changed in between, so an
/// index built for one is current for the other.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexVersion {
    cards: i64,
    cards_changed: Option<DateTime<Utc>>,
    printings: i64,
    printings_changed: Option<DateTime<Utc>>,
}

/// Reads cards, printings and rulings.
#[derive(Debug, Clone)]
pub struct CardRepo {
    pool: PgPool,
}

/// The stored UUID behind an oracle id; `None` for ids that cannot belong to any stored card.
fn uuid_of(id: &OracleId) -> Option<Uuid> {
    Uuid::parse_str(id.as_str()).ok()
}

impl CardRepo {
    pub(crate) fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Loads one card; ids that are unknown or malformed give `None`.
    pub async fn get(&self, id: &OracleId) -> Result<Option<Card>, DbError> {
        Ok(self.get_many(std::slice::from_ref(id)).await?.pop())
    }

    /// Loads the cards with the given ids ordered by name; unknown and malformed ids are omitted.
    pub async fn get_many(&self, ids: &[OracleId]) -> Result<Vec<Card>, DbError> {
        let uuids: Vec<Uuid> = ids.iter().filter_map(uuid_of).collect();
        let rows = sqlx::query_as!(
            CardRow,
            r#"SELECT oracle_id, name, layout, is_deck_card, mana_cost, mana_value, type_line,
                      oracle_text, keywords, colors, color_identity, produced_mana,
                      produces_colorless, legalities AS "legalities: _", faces AS "faces: _",
                      impl_status AS "impl_status: _"
               FROM cards WHERE oracle_id = ANY($1)
               ORDER BY name_normalized, oracle_id"#,
            &uuids
        )
        .fetch_all(&self.pool)
        .await?;
        rows.into_iter().map(Card::try_from).collect()
    }

    /// Loads a card with its printings (newest first) and rulings (oldest first).
    pub async fn detail(&self, id: &OracleId) -> Result<Option<CardDetail>, DbError> {
        let Some(card) = self.get(id).await? else {
            return Ok(None);
        };
        let uuid = uuid_of(id);
        let printings = sqlx::query_as!(
            PrintingRow,
            r#"SELECT printing_id, oracle_id, set_code, collector_number, rarity, released_at,
                      lang, digital, images AS "images: _"
               FROM printings WHERE oracle_id = $1
               ORDER BY released_at DESC NULLS LAST, printing_id"#,
            uuid
        )
        .fetch_all(&self.pool)
        .await?
        .into_iter()
        .map(Printing::try_from)
        .collect::<Result<Vec<_>, _>>()?;
        let rulings = sqlx::query_as!(
            RulingRow,
            "SELECT oracle_id, source, published_at, comment FROM rulings
             WHERE oracle_id = $1 ORDER BY published_at NULLS LAST, id",
            uuid
        )
        .fetch_all(&self.pool)
        .await?
        .into_iter()
        .map(Ruling::from)
        .collect();
        Ok(Some(CardDetail {
            card,
            printings,
            rulings,
        }))
    }

    /// A cheap fingerprint of the stored cards and printings, to tell whether a [`CardIndex`] built earlier is still current.
    pub async fn index_version(&self) -> Result<IndexVersion, DbError> {
        let row = sqlx::query!(
            r#"SELECT (SELECT count(*) FROM cards) AS "cards!",
                      (SELECT max(updated_at) FROM cards) AS cards_changed,
                      (SELECT count(*) FROM printings) AS "printings!",
                      (SELECT max(updated_at) FROM printings) AS printings_changed"#
        )
        .fetch_one(&self.pool)
        .await?;
        Ok(IndexVersion {
            cards: row.cards,
            cards_changed: row.cards_changed,
            printings: row.printings,
            printings_changed: row.printings_changed,
        })
    }

    /// Builds the in-memory index over all deck cards and their stored printings, for resolving deck lists.
    ///
    /// Every printing is known by its set and number; the flavor name and the printed name of an
    /// English printing are other names of its card that select that printing, and a meld piece
    /// is known by the name of both halves, as for an index built from the records
    /// ([`CardIndex::from_records`]).
    pub async fn card_index(&self) -> Result<CardIndex, DbError> {
        let rows = sqlx::query!(
            "SELECT oracle_id, name, melds_into FROM cards
             WHERE is_deck_card ORDER BY name_normalized, oracle_id"
        )
        .fetch_all(&self.pool)
        .await?;
        let mut index = CardIndex::new();
        for row in &rows {
            index.insert(OracleId(row.oracle_id.to_string()), &row.name);
        }
        for row in &rows {
            if let Some(result) = &row.melds_into {
                index.add_melds_into(&OracleId(row.oracle_id.to_string()), result);
            }
        }
        let printings = sqlx::query!(
            "SELECT p.oracle_id, p.set_code, p.collector_number,
                    CASE WHEN p.lang = 'en' THEN p.flavor_name END AS flavor_name,
                    CASE WHEN p.lang = 'en' THEN p.printed_name END AS printed_name
             FROM printings p JOIN cards c USING (oracle_id)
             WHERE c.is_deck_card ORDER BY p.set_code, p.collector_number, p.printing_id"
        )
        .fetch_all(&self.pool)
        .await?;
        for row in printings {
            let card = OracleId(row.oracle_id.to_string());
            index.add_printing(&card, &row.set_code, &row.collector_number);
            for name in [&row.flavor_name, &row.printed_name].into_iter().flatten() {
                index.add_printing_name(&card, &row.set_code, &row.collector_number, name);
            }
        }
        Ok(index)
    }
}
