//! The server's own schema migrations, tracked apart from `mtg-db`'s.
//!
//! `mtg-db` records its migrations in sqlx's default `_sqlx_migrations` table. Sharing it would make
//! each crate reject the other's versions as unknown, so the server keeps its own table and the two
//! can migrate one database in any order, again and again.

use sqlx::PgPool;
use sqlx::migrate::{MigrateError, Migrator};

/// The table recording which server migrations ran; renaming it would make them all run again.
pub const MIGRATIONS_TABLE: &str = "_mtg_server_migrations";

/// The server's migrations (embedded at compile time) bound to [`MIGRATIONS_TABLE`].
pub fn migrator() -> Migrator {
    let mut migrator = sqlx::migrate!();
    migrator.dangerous_set_table_name(MIGRATIONS_TABLE);
    migrator
}

/// Applies pending server migrations; safe to call on every start and from several processes.
pub async fn migrate(pool: &PgPool) -> Result<(), MigrateError> {
    migrator().run(pool).await
}
