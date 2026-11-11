//! Printings for lists: the art of each card's default printing, and printings by id.

use std::collections::HashMap;

use mtg_import::ids::{OracleId, PrintingId};
use uuid::Uuid;

use super::CardRepo;
use crate::error::DbError;
use crate::model::{Printing, PrintingImages};
use crate::rows::PrintingRow;

impl CardRepo {
    /// The art of each card's default printing: paper before digital, then the newest.
    ///
    /// Cards with no printing, unknown ids and malformed ids are absent from the map.
    pub async fn default_images(
        &self,
        ids: &[OracleId],
    ) -> Result<HashMap<OracleId, PrintingImages>, DbError> {
        let uuids: Vec<Uuid> = ids
            .iter()
            .filter_map(|id| Uuid::parse_str(id.as_str()).ok())
            .collect();
        let rows = sqlx::query!(
            r#"SELECT DISTINCT ON (oracle_id) oracle_id, images AS "images: sqlx::types::Json<PrintingImages>"
               FROM printings WHERE oracle_id = ANY($1)
               ORDER BY oracle_id, digital, released_at DESC NULLS LAST, printing_id"#,
            &uuids
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .into_iter()
            .map(|row| (OracleId(row.oracle_id.to_string()), row.images.0))
            .collect())
    }

    /// The numbers lit up on the Attraction printings of the given cards (CR 717.1), as `(card, printing, lights)` with
    /// the lights as bits (bit `n` for the number `n`), the newest paper printing of each card first; printings
    /// without lights, unknown ids and malformed ids are absent.
    pub async fn attraction_lights(
        &self,
        ids: &[OracleId],
    ) -> Result<Vec<(OracleId, PrintingId, u8)>, DbError> {
        let uuids: Vec<Uuid> = ids
            .iter()
            .filter_map(|id| Uuid::parse_str(id.as_str()).ok())
            .collect();
        let rows = sqlx::query!(
            r#"SELECT oracle_id, printing_id, attraction_lights
               FROM printings WHERE oracle_id = ANY($1) AND attraction_lights <> 0
               ORDER BY oracle_id, digital, released_at DESC NULLS LAST, printing_id"#,
            &uuids
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .into_iter()
            .map(|row| {
                (
                    OracleId(row.oracle_id.to_string()),
                    PrintingId(row.printing_id.to_string()),
                    u8::try_from(row.attraction_lights).unwrap_or(0),
                )
            })
            .collect())
    }

    /// The printings with the given ids; unknown and malformed ids are omitted.
    pub async fn printings(&self, ids: &[PrintingId]) -> Result<Vec<Printing>, DbError> {
        let uuids: Vec<Uuid> = ids
            .iter()
            .filter_map(|id| Uuid::parse_str(id.as_str()).ok())
            .collect();
        sqlx::query_as!(
            PrintingRow,
            r#"SELECT printing_id, oracle_id, set_code, collector_number, rarity, released_at,
                      lang, digital, images AS "images: _"
               FROM printings WHERE printing_id = ANY($1)
               ORDER BY printing_id"#,
            &uuids
        )
        .fetch_all(&self.pool)
        .await?
        .into_iter()
        .map(Printing::try_from)
        .collect()
    }

    /// The printing of the set `set` (any case; stored lower case) with collector number `number`, if the database holds it.
    pub async fn printing_by_number(
        &self,
        set: &str,
        number: &str,
    ) -> Result<Option<Printing>, DbError> {
        sqlx::query_as!(
            PrintingRow,
            r#"SELECT printing_id, oracle_id, set_code, collector_number, rarity, released_at,
                      lang, digital, images AS "images: _"
               FROM printings WHERE set_code = lower($1) AND collector_number = $2
               ORDER BY digital, lang <> 'en', printing_id LIMIT 1"#,
            set,
            number
        )
        .fetch_optional(&self.pool)
        .await?
        .map(Printing::try_from)
        .transpose()
    }
}
