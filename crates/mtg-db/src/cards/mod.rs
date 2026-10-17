//! Card queries: lookup by id or name, fuzzy suggestions, filtered search and the name index.

mod lookup;
mod search;

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

    /// Builds the in-memory name index over all deck cards, for resolving deck lists.
    pub async fn card_index(&self) -> Result<CardIndex, DbError> {
        let rows = sqlx::query!(
            "SELECT oracle_id, name FROM cards WHERE is_deck_card ORDER BY name_normalized, oracle_id"
        )
        .fetch_all(&self.pool)
        .await?;
        let mut index = CardIndex::new();
        for row in rows {
            index.insert(OracleId(row.oracle_id.to_string()), &row.name);
        }
        Ok(index)
    }
}
