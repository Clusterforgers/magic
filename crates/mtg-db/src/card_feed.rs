//! The card feed: the runs of the daily card refresh (`card_refresh_runs`) and the coverage snapshots
//! (`coverage_snapshots`) that the Card support page shows as a headline, a trend and a feed of events.
//! A snapshot is stored only when it differs from the newest one, and only the newest two keep their
//! list of missing cards, so the table stays small however long the job runs.

use mtg_import::ids::OracleId;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use sqlx::types::Json;

use crate::error::DbError;

/// The most names a refresh run or a snapshot keeps of the cards it lists; the counts stay whole.
pub const MAX_LISTED: usize = 1_000;

/// How many snapshots keep their list of missing cards: the newest and the one before, for the diff.
const KEEP_MISSING: i64 = 2;

/// The id of a refresh run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, sqlx::Type)]
#[sqlx(transparent)]
pub struct RefreshId(pub i64);

/// How a refresh run ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "refresh_status", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum RefreshStatus {
    /// Still running.
    Running,
    /// Fetched, loaded what changed and took a snapshot.
    Succeeded,
    /// Fetched, but left loading for a later run because a game was running.
    Deferred,
    /// A step failed; `failures` says which.
    Failed,
}

/// A precon a refresh run added.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NewPrecon {
    /// The MTGJSON set code.
    pub code: String,
    /// The deck's name.
    pub name: String,
    /// The name of its set, when a stored printing names it.
    pub set_name: Option<String>,
}

/// What a finished refresh run records.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RefreshReport {
    /// The `updated_at` of Scryfall's Oracle Cards file the run saw.
    pub scryfall_updated_at: Option<String>,
    /// The MTGJSON build of the deck index the run saw.
    pub mtgjson_build: Option<String>,
    /// How many deck cards the run added.
    pub new_card_count: u32,
    /// Their names, at most [`MAX_LISTED`].
    pub new_cards: Vec<String>,
    /// The precons the run added.
    pub new_precons: Vec<NewPrecon>,
    /// The database held no deck card before the run, so what it added is no news.
    pub first_load: bool,
    /// One summary line per step that ran.
    pub steps: Vec<String>,
    /// One line per step that failed.
    pub failures: Vec<String>,
}

/// A stored refresh run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RefreshRun {
    /// The run's id.
    pub id: RefreshId,
    /// When it started, as `YYYY-MM-DDTHH:MM:SSZ` (UTC).
    pub started_at: String,
    /// When it ended, in the same form; none while it runs.
    pub finished_at: Option<String>,
    /// How it ended.
    pub status: RefreshStatus,
    /// The release that ran it, when known.
    pub release: Option<String>,
    /// What it recorded.
    pub report: RefreshReport,
}

/// How many cards are in each support state.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct StateCounts {
    /// Verified by the checker.
    pub verified: u32,
    /// A read draft or a trusted Forge translation.
    pub read: u32,
    /// A whole draft that runs while it waits for its first read.
    pub unverified: u32,
    /// A partial draft that waits for its first read and does not run.
    pub waiting: u32,
    /// Rules except some clauses, which the table plays by hand.
    pub partial: u32,
    /// No rules the engine runs: played by hand.
    pub manual: u32,
    /// Stale or unplayable.
    pub other: u32,
}

/// The cards among the most played up to one rank, and how many of them are verified and automatic.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RankBucket {
    /// The bucket, as `top 100`, `all ranked` or `no rank`.
    pub label: String,
    /// Cards in it.
    pub cards: u32,
    /// Verified cards in it.
    pub verified: u32,
    /// Cards in it that play automatically.
    pub automatic: u32,
}

/// A card that does not play automatically, with what holds it back first.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MissingCard {
    /// The oracle id.
    pub oracle_id: String,
    /// The card's name.
    pub name: String,
    /// The state: `waiting`, `partial`, `manual` or `other`.
    pub state: String,
    /// The first blocker in plain words.
    pub blocker: String,
    /// The first blocker as the coverage report labels it.
    pub detail: String,
    /// How many blockers hold it back.
    pub blockers: u32,
    /// Its EDHREC rank, when it has one.
    pub rank: Option<u32>,
}

/// A snapshot to store.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct NewSnapshot {
    /// The release that took it, when known.
    pub release: Option<String>,
    /// Commander-legal cards judged.
    pub cards: u32,
    /// Of them, the cards that play automatically.
    pub automatic: u32,
    /// The cards per state.
    pub states: StateCounts,
    /// The popularity buckets.
    pub buckets: Vec<RankBucket>,
    /// Every card that does not play automatically, in rank order.
    pub missing: Vec<MissingCard>,
    /// How many cards play automatically now and did not in the newest stored snapshot.
    pub newly_supported_count: u32,
    /// Their names, at most [`MAX_LISTED`].
    pub newly_supported: Vec<String>,
}

/// A stored snapshot without its list of missing cards.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Snapshot {
    /// The snapshot's id.
    pub id: i64,
    /// When it was taken, as `YYYY-MM-DDTHH:MM:SSZ` (UTC).
    pub taken_at: String,
    /// The release that took it, when known.
    pub release: Option<String>,
    /// Commander-legal cards judged.
    pub cards: u32,
    /// Of them, the cards that play automatically.
    pub automatic: u32,
    /// The cards per state.
    pub states: StateCounts,
    /// The popularity buckets.
    pub buckets: Vec<RankBucket>,
    /// How many cards do not play automatically.
    pub missing_count: u32,
    /// How many cards play automatically since the snapshot before.
    pub newly_supported_count: u32,
    /// Their names, at most [`MAX_LISTED`].
    pub newly_supported: Vec<String>,
}

/// What [`CardFeedRepo::store_snapshot`] did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stored {
    /// A new row with this id.
    New(i64),
    /// The newest row, with this id, already says the same; nothing was written.
    Unchanged(i64),
}

/// Reads and writes the refresh runs and the coverage snapshots.
#[derive(Debug, Clone)]
pub struct CardFeedRepo {
    pool: PgPool,
}

fn count(value: i32) -> u32 {
    u32::try_from(value).unwrap_or(0)
}

fn int(value: u32) -> i32 {
    i32::try_from(value).unwrap_or(i32::MAX)
}

fn listed<T: Clone>(items: &[T]) -> Vec<T> {
    items.iter().take(MAX_LISTED).cloned().collect()
}

impl CardFeedRepo {
    pub(crate) fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Records the start of a refresh run by `release`.
    pub async fn start_refresh(&self, release: Option<&str>) -> Result<RefreshId, DbError> {
        let id = sqlx::query_scalar!(
            r#"INSERT INTO card_refresh_runs (release) VALUES (NULLIF($1, '')) RETURNING id AS "id: RefreshId""#,
            release
        )
        .fetch_one(&self.pool)
        .await?;
        Ok(id)
    }

    /// Ends a run with `status` and what it found; a run that has ended is not changed again.
    pub async fn finish_refresh(
        &self,
        id: RefreshId,
        status: RefreshStatus,
        report: &RefreshReport,
    ) -> Result<(), DbError> {
        let status = if status == RefreshStatus::Running {
            RefreshStatus::Failed
        } else {
            status
        };
        sqlx::query!(
            r#"UPDATE card_refresh_runs
               SET finished_at = greatest(now(), started_at), status = $2,
                   scryfall_updated_at = $3, mtgjson_build = $4, new_card_count = $5, new_cards = $6,
                   new_precons = $7, first_load = $8, steps = $9, failures = $10
               WHERE id = $1 AND status = 'running'"#,
            id as RefreshId,
            status as RefreshStatus,
            report.scryfall_updated_at,
            report.mtgjson_build,
            int(report.new_card_count),
            Json(listed(&report.new_cards)) as Json<Vec<String>>,
            Json(&report.new_precons) as Json<&Vec<NewPrecon>>,
            report.first_load,
            Json(&report.steps) as Json<&Vec<String>>,
            Json(&report.failures) as Json<&Vec<String>>,
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// The newest `limit` runs, the newest first.
    pub async fn refreshes(&self, limit: u32) -> Result<Vec<RefreshRun>, DbError> {
        let rows = sqlx::query!(
            r#"SELECT id AS "id: RefreshId", status AS "status: RefreshStatus", release,
                   to_char(started_at AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS"Z"') AS "started_at!",
                   to_char(finished_at AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS"Z"') AS finished_at,
                   scryfall_updated_at, mtgjson_build, new_card_count, first_load,
                   new_cards AS "new_cards: Json<Vec<String>>",
                   new_precons AS "new_precons: Json<Vec<NewPrecon>>",
                   steps AS "steps: Json<Vec<String>>", failures AS "failures: Json<Vec<String>>"
               FROM card_refresh_runs ORDER BY started_at DESC, id DESC LIMIT $1"#,
            i64::from(limit)
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .into_iter()
            .map(|row| RefreshRun {
                id: row.id,
                started_at: row.started_at,
                finished_at: row.finished_at,
                status: row.status,
                release: row.release,
                report: RefreshReport {
                    scryfall_updated_at: row.scryfall_updated_at,
                    mtgjson_build: row.mtgjson_build,
                    new_card_count: count(row.new_card_count),
                    new_cards: row.new_cards.0,
                    new_precons: row.new_precons.0,
                    first_load: row.first_load,
                    steps: row.steps.0,
                    failures: row.failures.0,
                },
            })
            .collect())
    }

    /// Stores `snapshot` unless the newest one has the same release, counts, buckets and missing
    /// cards; then only the newest two snapshots keep their missing cards.
    pub async fn store_snapshot(&self, snapshot: &NewSnapshot) -> Result<Stored, DbError> {
        let mut tx = self.pool.begin().await?;
        // One writer at a time, so two jobs never both see the same newest row.
        sqlx::query!("LOCK TABLE coverage_snapshots IN SHARE ROW EXCLUSIVE MODE")
            .execute(&mut *tx)
            .await?;
        let same = sqlx::query_scalar!(
            r#"SELECT id FROM (
                   SELECT id, release, cards, automatic, states, buckets, missing
                   FROM coverage_snapshots ORDER BY taken_at DESC, id DESC LIMIT 1
               ) newest
               WHERE release IS NOT DISTINCT FROM NULLIF($1, '') AND cards = $2 AND automatic = $3
                 AND states = $4 AND buckets = $5 AND missing = $6"#,
            snapshot.release,
            int(snapshot.cards),
            int(snapshot.automatic),
            Json(&snapshot.states) as Json<&StateCounts>,
            Json(&snapshot.buckets) as Json<&Vec<RankBucket>>,
            Json(&snapshot.missing) as Json<&Vec<MissingCard>>,
        )
        .fetch_optional(&mut *tx)
        .await?;
        if let Some(id) = same {
            tx.commit().await?;
            return Ok(Stored::Unchanged(id));
        }
        let id = sqlx::query_scalar!(
            r#"INSERT INTO coverage_snapshots
                   (release, cards, automatic, states, buckets, missing_count, missing,
                    newly_supported_count, newly_supported)
               VALUES (NULLIF($1, ''), $2, $3, $4, $5, $6, $7, $8, $9)
               RETURNING id"#,
            snapshot.release,
            int(snapshot.cards),
            int(snapshot.automatic),
            Json(&snapshot.states) as Json<&StateCounts>,
            Json(&snapshot.buckets) as Json<&Vec<RankBucket>>,
            int(u32::try_from(snapshot.missing.len()).unwrap_or(u32::MAX)),
            Json(&snapshot.missing) as Json<&Vec<MissingCard>>,
            int(snapshot.newly_supported_count),
            Json(listed(&snapshot.newly_supported)) as Json<Vec<String>>,
        )
        .fetch_one(&mut *tx)
        .await?;
        sqlx::query!(
            "UPDATE coverage_snapshots SET missing = NULL
             WHERE missing IS NOT NULL AND id NOT IN (
                 SELECT id FROM coverage_snapshots ORDER BY taken_at DESC, id DESC LIMIT $1
             )",
            KEEP_MISSING
        )
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(Stored::New(id))
    }

    /// The newest `limit` snapshots without their missing cards, the newest first.
    pub async fn snapshots(&self, limit: u32) -> Result<Vec<Snapshot>, DbError> {
        let rows = sqlx::query!(
            r#"SELECT id, release, cards, automatic, missing_count, newly_supported_count,
                   to_char(taken_at AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS"Z"') AS "taken_at!",
                   states AS "states: Json<StateCounts>", buckets AS "buckets: Json<Vec<RankBucket>>",
                   newly_supported AS "newly_supported: Json<Vec<String>>"
               FROM coverage_snapshots ORDER BY taken_at DESC, id DESC LIMIT $1"#,
            i64::from(limit)
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .into_iter()
            .map(|row| Snapshot {
                id: row.id,
                taken_at: row.taken_at,
                release: row.release,
                cards: count(row.cards),
                automatic: count(row.automatic),
                states: row.states.0,
                buckets: row.buckets.0,
                missing_count: count(row.missing_count),
                newly_supported_count: count(row.newly_supported_count),
                newly_supported: row.newly_supported.0,
            })
            .collect())
    }

    /// The missing cards of the newest snapshot with its id; none before the first snapshot.
    pub async fn latest_missing(&self) -> Result<Option<(i64, Vec<MissingCard>)>, DbError> {
        let row = sqlx::query!(
            r#"SELECT id, missing AS "missing: Json<Vec<MissingCard>>"
               FROM coverage_snapshots ORDER BY taken_at DESC, id DESC LIMIT 1"#
        )
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(|row| (row.id, row.missing.map(|list| list.0).unwrap_or_default())))
    }

    /// Every stored deck card as its oracle id and name.
    pub async fn deck_cards(&self) -> Result<Vec<(OracleId, String)>, DbError> {
        let rows = sqlx::query!("SELECT oracle_id, name FROM cards WHERE is_deck_card")
            .fetch_all(&self.pool)
            .await?;
        Ok(rows
            .into_iter()
            .map(|row| (OracleId(row.oracle_id.to_string()), row.name))
            .collect())
    }

    /// Every stored precon as its code and name.
    pub async fn precon_names(&self) -> Result<Vec<(String, String)>, DbError> {
        let rows = sqlx::query!("SELECT code, name FROM precon_decks")
            .fetch_all(&self.pool)
            .await?;
        Ok(rows.into_iter().map(|row| (row.code, row.name)).collect())
    }

    /// The set name a stored printing gives the set `code` (any case).
    pub async fn set_name(&self, code: &str) -> Result<Option<String>, DbError> {
        let name = sqlx::query_scalar!(
            "SELECT set_name FROM printings
             WHERE lower(set_code) = lower($1) AND set_name IS NOT NULL LIMIT 1",
            code
        )
        .fetch_optional(&self.pool)
        .await?;
        Ok(name.flatten())
    }

    /// How many games the server marks `running`; none when the database has no game table (a card-only one).
    pub async fn running_games(&self) -> Result<Option<u64>, DbError> {
        let exists: Option<bool> = sqlx::query_scalar("SELECT to_regclass('games') IS NOT NULL")
            .fetch_one(&self.pool)
            .await?;
        if exists != Some(true) {
            return Ok(None);
        }
        // Not a checked query: the table belongs to the server's schema, which this crate does not migrate.
        let running: i64 =
            sqlx::query_scalar("SELECT count(*) FROM games WHERE status = 'running'")
                .fetch_one(&self.pool)
                .await?;
        Ok(Some(u64::try_from(running).unwrap_or(0)))
    }
}
