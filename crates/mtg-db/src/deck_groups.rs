//! Deck groups: an account's named shelves for its decks, in the owner's order and each collapsed or
//! open. A deck sits in at most one group; deleting a group leaves its decks ungrouped. Every query
//! names the owner, so one account never reads or changes another's groups or files another's deck.

use std::collections::HashMap;

use sqlx::{PgConnection, PgPool};
use thiserror::Error;
use uuid::Uuid;

use crate::decks::DeckId;
use crate::error::DbError;

/// First half of the advisory-lock key that serialises the group writes of one owner ("MTGG").
const OWNER_LOCK_CLASS: i32 = 0x4d54_4747;

/// A group as stored, with the decks in it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredGroup {
    /// Identifier of the group.
    pub id: Uuid,
    /// The group's name.
    pub name: String,
    /// Whether the owner keeps the group collapsed.
    pub collapsed: bool,
    /// The decks in the group, by id.
    pub decks: Vec<DeckId>,
}

/// Why a group could not be created.
#[derive(Debug, Error)]
pub enum CreateGroupError {
    /// The owner already holds as many groups as allowed.
    #[error("the owner already holds {max} deck groups")]
    LimitReached {
        /// The cap that was met.
        max: u32,
    },
    /// The database failed.
    #[error(transparent)]
    Database(#[from] DbError),
}

impl From<sqlx::Error> for CreateGroupError {
    fn from(error: sqlx::Error) -> Self {
        CreateGroupError::Database(error.into())
    }
}

/// What [`DeckGroupRepo::file`] did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Filing {
    /// The deck sits in the group now, or in none.
    Done,
    /// The owner holds no such deck.
    NoDeck,
    /// The owner holds no such group.
    NoGroup,
}

/// A change to a group; `None` keeps that part.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GroupChange {
    /// The new name.
    pub name: Option<String>,
    /// Whether the group is collapsed now.
    pub collapsed: Option<bool>,
}

/// Reads and writes deck groups.
#[derive(Debug, Clone)]
pub struct DeckGroupRepo {
    pool: PgPool,
}

async fn lock(tx: &mut PgConnection, owner: Uuid) -> Result<(), DbError> {
    sqlx::query!(
        "SELECT pg_advisory_xact_lock($1, hashtext($2::text))",
        OWNER_LOCK_CLASS,
        owner.to_string()
    )
    .execute(&mut *tx)
    .await?;
    Ok(())
}

async fn load(conn: &mut PgConnection, owner: Uuid) -> Result<Vec<StoredGroup>, DbError> {
    let groups = sqlx::query!(
        "SELECT id, name, collapsed FROM deck_groups WHERE owner = $1 ORDER BY position, id",
        owner
    )
    .fetch_all(&mut *conn)
    .await?;
    let members = sqlx::query!(
        "SELECT m.group_id, m.deck_id FROM deck_group_members m
         JOIN deck_groups g ON g.id = m.group_id
         JOIN decks d ON d.id = m.deck_id
         WHERE g.owner = $1 ORDER BY d.updated_at DESC, d.id",
        owner
    )
    .fetch_all(&mut *conn)
    .await?;
    let mut by_group: HashMap<Uuid, Vec<DeckId>> = HashMap::new();
    for row in members {
        by_group
            .entry(row.group_id)
            .or_default()
            .push(DeckId(row.deck_id));
    }
    Ok(groups
        .into_iter()
        .map(|row| StoredGroup {
            decks: by_group.remove(&row.id).unwrap_or_default(),
            id: row.id,
            name: row.name,
            collapsed: row.collapsed,
        })
        .collect())
}

impl DeckGroupRepo {
    pub(crate) fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// The groups of `owner` in the owner's order, each with its decks, the most recently saved first.
    pub async fn list(&self, owner: Uuid) -> Result<Vec<StoredGroup>, DbError> {
        let mut conn = self.pool.acquire().await?;
        load(&mut conn, owner).await
    }

    /// Adds an empty, open group named `name` after the owner's last one, unless the owner holds `max`
    /// groups already.
    pub async fn create(
        &self,
        owner: Uuid,
        name: &str,
        max: u32,
    ) -> Result<StoredGroup, CreateGroupError> {
        let mut tx = self.pool.begin().await?;
        lock(&mut tx, owner).await?;
        let held = sqlx::query_scalar!(
            r#"SELECT count(*) AS "count!" FROM deck_groups WHERE owner = $1"#,
            owner
        )
        .fetch_one(&mut *tx)
        .await?;
        if held >= i64::from(max) {
            return Err(CreateGroupError::LimitReached { max });
        }
        let id = sqlx::query_scalar!(
            "INSERT INTO deck_groups (owner, name, position)
             SELECT $1, $2, COALESCE(max(position) + 1, 0) FROM deck_groups WHERE owner = $1
             RETURNING id",
            owner,
            name
        )
        .fetch_one(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(StoredGroup {
            id,
            name: name.to_owned(),
            collapsed: false,
            decks: Vec::new(),
        })
    }

    /// Renames, collapses or opens the group `id` of `owner`; `None` if the owner holds no such group.
    pub async fn change(
        &self,
        owner: Uuid,
        id: Uuid,
        change: &GroupChange,
    ) -> Result<Option<StoredGroup>, DbError> {
        let mut tx = self.pool.begin().await?;
        let changed = sqlx::query!(
            "UPDATE deck_groups SET name = COALESCE($3, name), collapsed = COALESCE($4, collapsed)
             WHERE owner = $1 AND id = $2",
            owner,
            id,
            change.name.as_deref(),
            change.collapsed
        )
        .execute(&mut *tx)
        .await?;
        if changed.rows_affected() == 0 {
            return Ok(None);
        }
        let group = load(&mut tx, owner)
            .await?
            .into_iter()
            .find(|group| group.id == id);
        tx.commit().await?;
        Ok(group)
    }

    /// Deletes the group `id` of `owner`; its decks are ungrouped. False if the owner holds no such group.
    pub async fn delete(&self, owner: Uuid, id: Uuid) -> Result<bool, DbError> {
        let deleted = sqlx::query!(
            "DELETE FROM deck_groups WHERE owner = $1 AND id = $2",
            owner,
            id
        )
        .execute(&self.pool)
        .await?;
        Ok(deleted.rows_affected() > 0)
    }

    /// Puts the groups of `owner` in the order of `ids`, which must name each of them exactly once;
    /// false (nothing changed) otherwise.
    pub async fn reorder(&self, owner: Uuid, ids: &[Uuid]) -> Result<bool, DbError> {
        let mut tx = self.pool.begin().await?;
        lock(&mut tx, owner).await?;
        let mut held = sqlx::query_scalar!("SELECT id FROM deck_groups WHERE owner = $1", owner)
            .fetch_all(&mut *tx)
            .await?;
        let mut asked = ids.to_vec();
        held.sort_unstable();
        asked.sort_unstable();
        if held != asked {
            return Ok(false);
        }
        sqlx::query!(
            "UPDATE deck_groups g SET position = o.position - 1
             FROM unnest($2::uuid[]) WITH ORDINALITY AS o(id, position)
             WHERE g.owner = $1 AND g.id = o.id",
            owner,
            ids
        )
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(true)
    }

    /// Puts the deck `deck` of `owner` in the group `group` of `owner`, or in none.
    pub async fn file(
        &self,
        owner: Uuid,
        deck: DeckId,
        group: Option<Uuid>,
    ) -> Result<Filing, DbError> {
        let mut tx = self.pool.begin().await?;
        let owned = sqlx::query_scalar!(
            r#"SELECT EXISTS (SELECT 1 FROM decks WHERE owner = $1 AND id = $2) AS "owned!""#,
            owner,
            deck.0
        )
        .fetch_one(&mut *tx)
        .await?;
        if !owned {
            return Ok(Filing::NoDeck);
        }
        let Some(group) = group else {
            sqlx::query!("DELETE FROM deck_group_members WHERE deck_id = $1", deck.0)
                .execute(&mut *tx)
                .await?;
            tx.commit().await?;
            return Ok(Filing::Done);
        };
        let filed = sqlx::query!(
            "INSERT INTO deck_group_members (deck_id, group_id)
             SELECT $2, id FROM deck_groups WHERE owner = $1 AND id = $3
             ON CONFLICT (deck_id) DO UPDATE SET group_id = EXCLUDED.group_id",
            owner,
            deck.0,
            group
        )
        .execute(&mut *tx)
        .await?;
        if filed.rows_affected() == 0 {
            return Ok(Filing::NoGroup);
        }
        tx.commit().await?;
        Ok(Filing::Done)
    }

    /// Deletes every group of `owner`, inside the caller's transaction `tx`, and returns how many went. A
    /// database whose group table is not migrated yet holds none: 0.
    pub async fn delete_all_of(tx: &mut PgConnection, owner: Uuid) -> Result<u64, DbError> {
        let migrated =
            sqlx::query_scalar!(r#"SELECT to_regclass('deck_groups') IS NOT NULL AS "migrated!""#)
                .fetch_one(&mut *tx)
                .await?;
        if !migrated {
            return Ok(0);
        }
        let deleted = sqlx::query!("DELETE FROM deck_groups WHERE owner = $1", owner)
            .execute(&mut *tx)
            .await?;
        Ok(deleted.rows_affected())
    }
}
