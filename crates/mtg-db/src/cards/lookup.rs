//! Lookup by normalized name and trigram-based name suggestions.

use mtg_import::ids::OracleId;
use mtg_import::resolve::normalize_name;

use super::CardRepo;
use crate::error::DbError;
use crate::model::Card;
use crate::rows::CardRow;

/// Minimum whole-name trigram similarity for a suggestion.
const SIMILARITY_THRESHOLD: &str = "0.3";

/// Minimum similarity of the query to a part of a name (for partial and face-only queries).
const WORD_SIMILARITY_THRESHOLD: &str = "0.6";

/// Most suggestions a single call returns.
pub const MAX_SUGGESTIONS: u32 = 50;

/// A card name close to a query, with its trigram similarity (0 to 1, higher is closer).
#[derive(Debug, Clone, PartialEq)]
pub struct NameSuggestion {
    /// The suggested card.
    pub oracle_id: OracleId,
    /// Canonical card name.
    pub name: String,
    /// Best of whole-name and partial-name similarity.
    pub similarity: f32,
}

impl CardRepo {
    /// Deck cards whose normalized full name equals `name`, else those whose front face does.
    ///
    /// Empty means unknown, one card is a match, several cards share the name.
    pub async fn find_by_name(&self, name: &str) -> Result<Vec<Card>, DbError> {
        let key = normalize_name(name);
        if key.is_empty() {
            return Ok(Vec::new());
        }
        let full = sqlx::query_as!(
            CardRow,
            r#"SELECT oracle_id, name, layout, is_deck_card, mana_cost, mana_value, type_line,
                      oracle_text, keywords, colors, color_identity, produced_mana,
                      produces_colorless, legalities AS "legalities: _", faces AS "faces: _",
                      impl_status AS "impl_status: _"
               FROM cards WHERE is_deck_card AND name_normalized = $1
               ORDER BY oracle_id"#,
            key
        )
        .fetch_all(&self.pool)
        .await?;
        if !full.is_empty() {
            return full.into_iter().map(Card::try_from).collect();
        }
        let front = sqlx::query_as!(
            CardRow,
            r#"SELECT oracle_id, name, layout, is_deck_card, mana_cost, mana_value, type_line,
                      oracle_text, keywords, colors, color_identity, produced_mana,
                      produces_colorless, legalities AS "legalities: _", faces AS "faces: _",
                      impl_status AS "impl_status: _"
               FROM cards WHERE is_deck_card AND front_name_normalized = $1
               ORDER BY oracle_id"#,
            key
        )
        .fetch_all(&self.pool)
        .await?;
        front.into_iter().map(Card::try_from).collect()
    }

    /// Up to `limit` deck-card names close to `query`, best first.
    ///
    /// Ranks exact matches, then prefix matches, then by trigram similarity, so typos,
    /// partial names and single faces of multi-faced cards all find their card.
    pub async fn suggest(&self, query: &str, limit: u32) -> Result<Vec<NameSuggestion>, DbError> {
        let key = normalize_name(query);
        if key.is_empty() {
            return Ok(Vec::new());
        }
        let limit = i64::from(limit.clamp(1, MAX_SUGGESTIONS));
        let mut tx = self.pool.begin().await?;
        // Pinned per transaction so the `%` and `<%` operators (which use the GIN index) do not
        // depend on server-wide pg_trgm settings.
        sqlx::query!(
            r#"SELECT set_config('pg_trgm.similarity_threshold', $1, true) AS "similarity!",
                      set_config('pg_trgm.word_similarity_threshold', $2, true) AS "word_similarity!""#,
            SIMILARITY_THRESHOLD,
            WORD_SIMILARITY_THRESHOLD
        )
        .fetch_one(&mut *tx)
        .await?;
        let rows = sqlx::query!(
            r#"SELECT oracle_id, name,
                      GREATEST(similarity(name_normalized, $1), word_similarity($1, name_normalized))
                          AS "similarity!"
               FROM cards
               WHERE is_deck_card AND (name_normalized % $1 OR $1 <% name_normalized)
               ORDER BY (name_normalized = $1) DESC,
                        starts_with(name_normalized, $1) DESC,
                        "similarity!" DESC,
                        name_normalized,
                        oracle_id
               LIMIT $2"#,
            key,
            limit
        )
        .fetch_all(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(rows
            .into_iter()
            .map(|row| NameSuggestion {
                oracle_id: OracleId(row.oracle_id.to_string()),
                name: row.name,
                similarity: row.similarity,
            })
            .collect())
    }
}
