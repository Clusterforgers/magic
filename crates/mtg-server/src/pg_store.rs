//! A Postgres-backed [`EventStore`]: one `game_events` row per engine event.

use mtg_engine::Event;
use serde_json::Value;
use sqlx::PgPool;

use crate::game_id::GameId;
use crate::store::{EventStore, StoreError};

/// First half of the advisory-lock key that serialises appends to one game ("MTGE").
const APPEND_LOCK_CLASS: i32 = 0x4d54_4745;

/// Event logs in Postgres; clones share the pool. Run [`migrate`](crate::migrate) before use.
///
/// Each event is one jsonb row keyed by `(game_id, seq)`. An append is a single transaction that
/// holds a per-game advisory lock, so concurrent appends to one game take turns and all but the
/// first see the log has moved on.
#[derive(Debug, Clone)]
pub struct PgEventStore {
    pool: PgPool,
}

impl PgEventStore {
    /// A store over `pool`.
    pub fn new(pool: PgPool) -> Self {
        PgEventStore { pool }
    }

    /// The game's events from sequence number `from_seq` on, in order; empty when it has none there.
    pub async fn read_from(&self, game: GameId, from_seq: u64) -> Result<Vec<Event>, StoreError> {
        // A start past the largest bigint cannot hold a row, and clamping keeps the query valid.
        let start = i64::try_from(from_seq).unwrap_or(i64::MAX);
        let rows = sqlx::query!(
            "SELECT seq, event FROM game_events WHERE game_id = $1 AND seq >= $2 ORDER BY seq",
            game.uuid(),
            start,
        )
        .fetch_all(&self.pool)
        .await?;

        let mut events = Vec::with_capacity(rows.len());
        let mut expected = from_seq;
        for row in rows {
            if u64::try_from(row.seq) != Ok(expected) {
                return Err(StoreError::Corrupt(format!(
                    "game {game}: expected event {expected}, found {}",
                    row.seq
                )));
            }
            let event = serde_json::from_value(row.event).map_err(|e| {
                StoreError::Corrupt(format!(
                    "game {game}: event {expected} does not decode: {e}"
                ))
            })?;
            events.push(event);
            expected = expected.saturating_add(1);
        }
        Ok(events)
    }

    async fn append_now(
        &self,
        game: GameId,
        from_seq: u64,
        events: &[Event],
    ) -> Result<(), StoreError> {
        let payload = events
            .iter()
            .map(serde_json::to_value)
            .collect::<Result<Vec<Value>, _>>()
            .map_err(|e| StoreError::Corrupt(format!("game {game}: event does not encode: {e}")))?;

        let mut tx = self.pool.begin().await?;
        sqlx::query!(
            "SELECT pg_advisory_xact_lock($1, hashtext($2::text))",
            APPEND_LOCK_CLASS,
            game.to_string(),
        )
        .execute(&mut *tx)
        .await?;

        let stored = sqlx::query_scalar!(
            r#"SELECT coalesce(max(seq) + 1, 0) AS "stored!" FROM game_events WHERE game_id = $1"#,
            game.uuid(),
        )
        .fetch_one(&mut *tx)
        .await?;
        let stored = u64::try_from(stored).unwrap_or(u64::MAX);
        if stored != from_seq {
            return Err(StoreError::OutOfOrder { from_seq, stored });
        }

        if !payload.is_empty() {
            // `stored == from_seq` and rows exist, so the sequence fits a bigint.
            let start = i64::try_from(from_seq).unwrap_or(i64::MAX);
            sqlx::query!(
                "INSERT INTO game_events (game_id, seq, event)
                 SELECT $1, $2 + t.ord - 1, t.event
                 FROM unnest($3::jsonb[]) WITH ORDINALITY AS t(event, ord)",
                game.uuid(),
                start,
                &payload[..],
            )
            .execute(&mut *tx)
            .await?;
        }
        Ok(tx.commit().await?)
    }
}

impl From<sqlx::Error> for StoreError {
    fn from(error: sqlx::Error) -> Self {
        StoreError::Unavailable(error.to_string())
    }
}

impl EventStore for PgEventStore {
    async fn append(
        &self,
        game: GameId,
        from_seq: u64,
        events: &[Event],
    ) -> Result<(), StoreError> {
        self.append_now(game, from_seq, events).await
    }

    async fn load(&self, game: GameId) -> Result<Vec<Event>, StoreError> {
        self.read_from(game, 0).await
    }
}
