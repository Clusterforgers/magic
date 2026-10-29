//! Connection pool, migrations and access to the repositories.

use std::time::Duration;

use sqlx::PgPool;
use sqlx::migrate::Migrator;
use sqlx::postgres::PgPoolOptions;

use crate::card_rules::CardRulesRepo;
use crate::cards::CardRepo;
use crate::decks::DeckRepo;
use crate::error::DbError;
use crate::fix_marks::FixMarkRepo;
use crate::import::Importer;
use crate::precons::PreconRepo;
use crate::runs::ImportRuns;

/// Migrations embedded at compile time from `migrations/`.
static MIGRATOR: Migrator = sqlx::migrate!();

/// Pool size used by [`Database::connect`].
const MAX_CONNECTIONS: u32 = 10;

/// How long a connection attempt may wait for a free pooled connection.
const ACQUIRE_TIMEOUT: Duration = Duration::from_secs(10);

/// A connection pool with the card-data repositories on top.
#[derive(Debug, Clone)]
pub struct Database {
    pool: PgPool,
}

impl Database {
    /// Connects to the Postgres server at `url`; call [`Database::migrate`] before use.
    pub async fn connect(url: &str) -> Result<Self, DbError> {
        let pool = PgPoolOptions::new()
            .max_connections(MAX_CONNECTIONS)
            .acquire_timeout(ACQUIRE_TIMEOUT)
            .connect(url)
            .await?;
        Ok(Self { pool })
    }

    /// Wraps an existing pool, for callers that manage connections themselves.
    pub fn from_pool(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Applies pending migrations; safe to call on every start and from several processes.
    pub async fn migrate(&self) -> Result<(), DbError> {
        MIGRATOR.run(&self.pool).await?;
        Ok(())
    }

    /// The underlying pool.
    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    /// Card queries: lookup, suggestions, search and the name index.
    pub fn cards(&self) -> CardRepo {
        CardRepo::new(self.pool.clone())
    }

    /// Rules the compiler generated for cards.
    pub fn card_rules(&self) -> CardRulesRepo {
        CardRulesRepo::new(self.pool.clone())
    }

    /// Decks stored by accounts.
    pub fn decks(&self) -> DeckRepo {
        DeckRepo::new(self.pool.clone())
    }

    /// Cards accounts marked as important to fix.
    pub fn fix_marks(&self) -> FixMarkRepo {
        FixMarkRepo::new(self.pool.clone())
    }

    /// Precon deck listing and loading.
    pub fn precons(&self) -> PreconRepo {
        PreconRepo::new(self.pool.clone())
    }

    /// Import run history.
    pub fn import_runs(&self) -> ImportRuns {
        ImportRuns::new(self.pool.clone())
    }

    /// Bulk ingestion of cards, rulings and precon decks.
    pub fn importer(&self) -> Importer {
        Importer::new(self.pool.clone())
    }

    /// How many rows each card-data table holds.
    pub async fn row_counts(&self) -> Result<RowCounts, DbError> {
        let row = sqlx::query!(
            r#"SELECT (SELECT count(*) FROM cards) AS "cards!",
                      (SELECT count(*) FROM printings) AS "printings!",
                      (SELECT count(*) FROM rulings) AS "rulings!",
                      (SELECT count(*) FROM precon_decks) AS "precon_decks!",
                      (SELECT count(*) FROM precon_deck_cards) AS "precon_entries!""#
        )
        .fetch_one(&self.pool)
        .await?;
        Ok(RowCounts {
            cards: row.cards.unsigned_abs(),
            printings: row.printings.unsigned_abs(),
            rulings: row.rulings.unsigned_abs(),
            precon_decks: row.precon_decks.unsigned_abs(),
            precon_entries: row.precon_entries.unsigned_abs(),
        })
    }
}

/// Rows per card-data table.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RowCounts {
    /// Oracle cards.
    pub cards: u64,
    /// Printings.
    pub printings: u64,
    /// Rulings.
    pub rulings: u64,
    /// Precon decks.
    pub precon_decks: u64,
    /// Lines of precon decks.
    pub precon_entries: u64,
}
