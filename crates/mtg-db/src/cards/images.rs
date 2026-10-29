//! The printing whose pictures stand for a card or a token.

use mtg_import::ids::{OracleId, PrintingId};
use mtg_import::resolve::normalize_name;
use sqlx::types::Json;
use uuid::Uuid;

use super::{CardRepo, uuid_of};
use crate::error::DbError;
use crate::model::{Card, PrintingImages};

/// A printing with the pictures it has.
#[derive(Debug, Clone, PartialEq)]
pub struct PrintingPictures {
    /// The printing the pictures belong to.
    pub printing_id: PrintingId,
    /// Its card-level and per-face pictures.
    pub images: PrintingImages,
}

/// A token card with the printing that shows it.
#[derive(Debug, Clone, PartialEq)]
pub struct TokenPictures {
    /// The token's oracle record: type line, colors and the faces' power and toughness.
    pub card: Card,
    /// The newest printing of the token that has a picture.
    pub pictures: PrintingPictures,
}

impl CardRepo {
    /// The printing that stands for the card: the newest paper printing that has a picture, else
    /// the newest digital one. `None` for an unknown or malformed id and for a card no printing
    /// of which has a picture.
    pub async fn picture_printing(
        &self,
        id: &OracleId,
    ) -> Result<Option<PrintingPictures>, DbError> {
        let Some(uuid) = uuid_of(id) else {
            return Ok(None);
        };
        let row = sqlx::query!(
            r#"SELECT printing_id, images AS "images: Json<PrintingImages>"
               FROM printings
               WHERE oracle_id = $1 AND jsonb_path_exists(images, '$.** ? (@.type() == "string")')
               ORDER BY digital, lang <> 'en', released_at DESC NULLS LAST, printing_id
               LIMIT 1"#,
            uuid
        )
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(|row| PrintingPictures {
            printing_id: PrintingId(row.printing_id.to_string()),
            images: row.images.0,
        }))
    }

    /// The tokens named `name` (case, accents and the second face's name ignored) that have a
    /// picture, each with the printing that shows it, by oracle id.
    pub async fn token_pictures(&self, name: &str) -> Result<Vec<TokenPictures>, DbError> {
        let key = normalize_name(name);
        if key.is_empty() {
            return Ok(Vec::new());
        }
        let rows = sqlx::query!(
            r#"SELECT DISTINCT ON (c.oracle_id) c.oracle_id AS "oracle_id: Uuid", p.printing_id,
                      p.images AS "images: Json<PrintingImages>"
               FROM cards c JOIN printings p ON p.oracle_id = c.oracle_id
               WHERE NOT c.is_deck_card AND c.layout IN ('token', 'double_faced_token')
                 AND (c.name_normalized = $1 OR c.front_name_normalized = $1)
                 AND jsonb_path_exists(p.images, '$.** ? (@.type() == "string")')
               ORDER BY c.oracle_id, p.digital, p.lang <> 'en', p.released_at DESC NULLS LAST,
                        p.printing_id"#,
            key
        )
        .fetch_all(&self.pool)
        .await?;
        let ids: Vec<OracleId> = rows
            .iter()
            .map(|row| OracleId(row.oracle_id.to_string()))
            .collect();
        let cards = self.get_many(&ids).await?;
        Ok(cards
            .into_iter()
            .filter_map(|card| {
                let row = rows
                    .iter()
                    .find(|row| row.oracle_id.to_string() == card.oracle_id.as_str())?;
                Some(TokenPictures {
                    pictures: PrintingPictures {
                        printing_id: PrintingId(row.printing_id.to_string()),
                        images: row.images.0.clone(),
                    },
                    card,
                })
            })
            .collect())
    }
}
