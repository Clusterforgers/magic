//! Account preferences: one row per account with the default stops, the end-turn-by-hand choice and the auto choices
//! (smart stops among them).
//! Every query names the account, so one account never reads or changes another's row.

use serde_json::Value;
use sqlx::{PgConnection, PgPool};
use uuid::Uuid;

use crate::error::DbError;

/// The stored preferences of one account.
// One flag per column: the settings are independent of each other.
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone, PartialEq)]
pub struct StoredPrefs {
    /// A JSON array of stops, as the server wrote it.
    pub default_stops: Value,
    /// Whether the account's seats end their own turns by hand.
    pub end_turn_by_hand: bool,
    /// Whether the account's seats have the server order their triggers by timestamp.
    pub auto_order_triggers: bool,
    /// Whether the account's seats have the server assign their combat damage.
    pub auto_assign_damage: bool,
    /// Whether the account's seats have smart stops.
    pub auto_smart_stops: bool,
}

impl Default for StoredPrefs {
    fn default() -> Self {
        Self {
            default_stops: Value::Array(Vec::new()),
            end_turn_by_hand: false,
            auto_order_triggers: false,
            auto_assign_damage: false,
            auto_smart_stops: false,
        }
    }
}

/// Reads and writes account preferences.
#[derive(Debug, Clone)]
pub struct PrefsRepo {
    pool: PgPool,
}

impl PrefsRepo {
    pub(crate) fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// The preferences of `user`; the defaults when none were saved.
    pub async fn get(&self, user: Uuid) -> Result<StoredPrefs, DbError> {
        let row = sqlx::query!(
            "SELECT default_stops, end_turn_by_hand, auto_order_triggers, auto_assign_damage, auto_smart_stops FROM user_prefs WHERE user_id = $1",
            user
        )
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map_or_else(StoredPrefs::default, |row| StoredPrefs {
            default_stops: row.default_stops,
            end_turn_by_hand: row.end_turn_by_hand,
            auto_order_triggers: row.auto_order_triggers,
            auto_assign_damage: row.auto_assign_damage,
            auto_smart_stops: row.auto_smart_stops,
        }))
    }

    /// Replaces the preferences of `user`.
    pub async fn put(&self, user: Uuid, prefs: &StoredPrefs) -> Result<(), DbError> {
        sqlx::query!(
            "INSERT INTO user_prefs (user_id, default_stops, end_turn_by_hand, auto_order_triggers, auto_assign_damage, auto_smart_stops)
             VALUES ($1, $2, $3, $4, $5, $6)
             ON CONFLICT (user_id) DO UPDATE
             SET default_stops = EXCLUDED.default_stops,
                 end_turn_by_hand = EXCLUDED.end_turn_by_hand,
                 auto_order_triggers = EXCLUDED.auto_order_triggers,
                 auto_assign_damage = EXCLUDED.auto_assign_damage,
                 auto_smart_stops = EXCLUDED.auto_smart_stops,
                 updated_at = now()",
            user,
            prefs.default_stops,
            prefs.end_turn_by_hand,
            prefs.auto_order_triggers,
            prefs.auto_assign_damage,
            prefs.auto_smart_stops
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Deletes the row of `user` inside the caller's transaction `tx`; 0 or 1, and 0 on a database not migrated yet.
    pub async fn delete_all_of(tx: &mut PgConnection, user: Uuid) -> Result<u64, DbError> {
        let migrated =
            sqlx::query_scalar!(r#"SELECT to_regclass('user_prefs') IS NOT NULL AS "migrated!""#)
                .fetch_one(&mut *tx)
                .await?;
        if !migrated {
            return Ok(0);
        }
        let deleted = sqlx::query!("DELETE FROM user_prefs WHERE user_id = $1", user)
            .execute(&mut *tx)
            .await?;
        Ok(deleted.rows_affected())
    }
}
