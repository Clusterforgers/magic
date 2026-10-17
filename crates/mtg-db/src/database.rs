//! Connection pool, migrations and access to the repositories.

use std::time::Duration;

use sqlx::PgPool;
use sqlx::migrate::Migrator;
use sqlx::postgres::PgPoolOptions;

use crate::cards::CardRepo;
use crate::error::DbError;
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
}
