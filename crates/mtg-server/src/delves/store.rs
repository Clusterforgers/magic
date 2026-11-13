//! The stored runs: each run's header and the log of its events, and the accounts in each party.

use mtg_campaign::RunEvent;
use sqlx::PgPool;
use uuid::Uuid;

use crate::records::RecordError;
use crate::user_id::UserId;

/// The Postgres store of the runs.
#[derive(Debug, Clone)]
pub(crate) struct DelveStore {
    pool: PgPool,
}

/// A run's header: what it was created with.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Header {
    /// The run's id.
    pub(crate) id: Uuid,
    /// The invite code.
    pub(crate) code: String,
    /// The host's account.
    pub(crate) host: Uuid,
    /// The theme's id.
    pub(crate) theme: String,
    /// The seed.
    pub(crate) seed: u64,
    /// The Heat.
    pub(crate) heat: u8,
}

impl DelveStore {
    /// A store over `pool`, whose server migrations have run.
    pub(crate) fn new(pool: PgPool) -> DelveStore {
        DelveStore { pool }
    }

    /// Writes a new run with no events yet.
    pub(crate) async fn create(&self, header: &Header) -> Result<(), RecordError> {
        sqlx::query!(
            "INSERT INTO delves (id, code, host, theme, seed, heat) VALUES ($1, $2, $3, $4, $5, $6)",
            header.id,
            header.code,
            header.host,
            header.theme,
            // The seed's bits as stored; read back the same way.
            i64::from_ne_bytes(header.seed.to_ne_bytes()),
            i16::from(header.heat),
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// A run's header.
    pub(crate) async fn header(&self, id: Uuid) -> Result<Option<Header>, RecordError> {
        let row = sqlx::query!(
            "SELECT id, code, host, theme, seed, heat FROM delves WHERE id = $1",
            id
        )
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(|row| Header {
            id: row.id,
            code: row.code,
            host: row.host,
            theme: row.theme,
            seed: u64::from_ne_bytes(row.seed.to_ne_bytes()),
            heat: u8::try_from(row.heat).unwrap_or(0),
        }))
    }

    /// The run an invite code opens.
    pub(crate) async fn by_code(&self, code: &str) -> Result<Option<Uuid>, RecordError> {
        let id = sqlx::query_scalar!("SELECT id FROM delves WHERE code = $1", code)
            .fetch_optional(&self.pool)
            .await?;
        Ok(id)
    }

    /// A run's events in order.
    pub(crate) async fn events(&self, id: Uuid) -> Result<Vec<RunEvent>, RecordError> {
        let rows = sqlx::query_scalar!(
            "SELECT event FROM delve_events WHERE delve_id = $1 ORDER BY seq",
            id
        )
        .fetch_all(&self.pool)
        .await?;
        rows.into_iter()
            .map(|value| {
                serde_json::from_value(value)
                    .map_err(|e| RecordError::Corrupt(format!("delve event: {e}")))
            })
            .collect()
    }

    /// Appends `events` after the first `from` events, with the run's status and party; a write that races another
    /// is refused by the log's key and changes nothing.
    pub(crate) async fn append(
        &self,
        id: Uuid,
        from: usize,
        events: &[RunEvent],
        status: &str,
        members: &[Uuid],
    ) -> Result<(), RecordError> {
        let mut tx = self.pool.begin().await?;
        for (at, event) in events.iter().enumerate() {
            let seq = i32::try_from(from + at)
                .map_err(|_| RecordError::Encode("too many delve events".into()))?;
            let value =
                serde_json::to_value(event).map_err(|e| RecordError::Encode(e.to_string()))?;
            sqlx::query!(
                "INSERT INTO delve_events (delve_id, seq, event) VALUES ($1, $2, $3)",
                id,
                seq,
                value,
            )
            .execute(&mut *tx)
            .await?;
        }
        sqlx::query!(
            "UPDATE delves SET status = $2, updated_at = now() WHERE id = $1",
            id,
            status
        )
        .execute(&mut *tx)
        .await?;
        sqlx::query!(
            "DELETE FROM delve_members WHERE delve_id = $1 AND NOT (user_id = ANY($2))",
            id,
            members
        )
        .execute(&mut *tx)
        .await?;
        sqlx::query!(
            "INSERT INTO delve_members (delve_id, user_id) SELECT $1, unnest($2::uuid[]) ON CONFLICT DO NOTHING",
            id,
            members
        )
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(())
    }

    /// The runs `user` hosts or plays in, newest first, at most `limit`.
    pub(crate) async fn list_for(
        &self,
        user: UserId,
        limit: i64,
    ) -> Result<Vec<Uuid>, RecordError> {
        let ids = sqlx::query_scalar!(
            "SELECT d.id FROM delves d
             WHERE d.host = $1 OR EXISTS (SELECT 1 FROM delve_members m WHERE m.delve_id = d.id AND m.user_id = $1)
             ORDER BY d.updated_at DESC
             LIMIT $2",
            user.uuid(),
            limit,
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(ids)
    }
}
