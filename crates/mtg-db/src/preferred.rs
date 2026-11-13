//! Preferred art: the printing an account plays a card in when a deck line chooses none, one row per account and
//! card. Every query names the account, so one account never reads or changes another's choice.

use std::collections::HashMap;

use mtg_import::ids::{OracleId, PrintingId};
use sqlx::{PgConnection, PgPool};
use uuid::Uuid;

use crate::error::DbError;

/// Reads and writes preferred printings.
#[derive(Debug, Clone)]
pub struct PreferredPrintingRepo {
    pool: PgPool,
}

impl PreferredPrintingRepo {
    pub(crate) fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Makes `printing` the art `owner` plays `oracle` in; false (nothing stored) when the printing does not exist
    /// or is one of another card.
    pub async fn set(
        &self,
        owner: Uuid,
        oracle: &OracleId,
        printing: &PrintingId,
    ) -> Result<bool, DbError> {
        let (Ok(card), Ok(chosen)) = (
            Uuid::parse_str(oracle.as_str()),
            Uuid::parse_str(printing.as_str()),
        ) else {
            return Ok(false);
        };
        let stored = sqlx::query!(
            "INSERT INTO preferred_printings (owner, oracle_id, printing_id)
             SELECT $1, oracle_id, printing_id FROM printings WHERE printing_id = $3 AND oracle_id = $2
             ON CONFLICT (owner, oracle_id) DO UPDATE SET printing_id = EXCLUDED.printing_id, updated_at = now()",
            owner,
            card,
            chosen
        )
        .execute(&self.pool)
        .await?;
        Ok(stored.rows_affected() > 0)
    }

    /// Forgets the art `owner` chose for `oracle`; false if there was none.
    pub async fn clear(&self, owner: Uuid, oracle: &OracleId) -> Result<bool, DbError> {
        let Ok(card) = Uuid::parse_str(oracle.as_str()) else {
            return Ok(false);
        };
        let removed = sqlx::query!(
            "DELETE FROM preferred_printings WHERE owner = $1 AND oracle_id = $2",
            owner,
            card
        )
        .execute(&self.pool)
        .await?;
        Ok(removed.rows_affected() > 0)
    }

    /// Every card `owner` chose art for, with that printing.
    pub async fn all(&self, owner: Uuid) -> Result<HashMap<OracleId, PrintingId>, DbError> {
        let rows = sqlx::query!(
            "SELECT oracle_id, printing_id FROM preferred_printings WHERE owner = $1",
            owner
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .into_iter()
            .map(|row| {
                (
                    OracleId(row.oracle_id.to_string()),
                    PrintingId(row.printing_id.to_string()),
                )
            })
            .collect())
    }

    /// Deletes every choice of `owner`, inside the caller's transaction `tx`, and returns how many went. A database
    /// whose table is not migrated yet holds none: 0.
    pub async fn delete_all_of(tx: &mut PgConnection, owner: Uuid) -> Result<u64, DbError> {
        let migrated = sqlx::query_scalar!(
            r#"SELECT to_regclass('preferred_printings') IS NOT NULL AS "migrated!""#
        )
        .fetch_one(&mut *tx)
        .await?;
        if !migrated {
            return Ok(0);
        }
        let deleted = sqlx::query!("DELETE FROM preferred_printings WHERE owner = $1", owner)
            .execute(&mut *tx)
            .await?;
        Ok(deleted.rows_affected())
    }
}
