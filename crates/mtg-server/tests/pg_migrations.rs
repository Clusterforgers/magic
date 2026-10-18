//! Goal: the server's migrations are tracked in a table of their own, so they and mtg-db's can migrate one database in either order, repeatedly and at the same time, without touching each other's records or data.
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use common::game_id;
use mtg_core::ids::PlayerId;
use mtg_db::Database;
use mtg_engine::Event;
use mtg_server::{EventStore, MIGRATIONS_TABLE, PgEventStore, migrate, migrator};
use sqlx::PgPool;
use sqlx::migrate::MigrateError;

/// The versions and checksums a migration table records, in order.
async fn recorded(pool: &PgPool, table: &str) -> Vec<(i64, Vec<u8>)> {
    // `table` is always one of this file's own constants.
    sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "SELECT version, checksum FROM {table} WHERE success ORDER BY version"
    )))
    .fetch_all(pool)
    .await
    .unwrap()
}

async fn tables(pool: &PgPool) -> Vec<String> {
    sqlx::query_scalar(
        "SELECT table_name::text FROM information_schema.tables
         WHERE table_schema = 'public' ORDER BY 1",
    )
    .fetch_all(pool)
    .await
    .unwrap()
}

fn shipped() -> usize {
    migrator().iter().count()
}

#[sqlx::test(migrations = false)]
async fn the_server_alone_creates_only_its_own_tables_and_can_rerun(pool: PgPool) {
    for _ in 0..3 {
        migrate(&pool).await.unwrap();
        assert_eq!(tables(&pool).await, [MIGRATIONS_TABLE, "game_events"]);
        let rows = recorded(&pool, MIGRATIONS_TABLE).await;
        assert_eq!(rows.len(), shipped());
        assert_ne!(rows.len(), 0);
    }
}

/// Migrates with `first` and then the other, twice over, checking each record stays as it was.
async fn both_in_order(pool: PgPool, server_first: bool) {
    let database = Database::from_pool(pool.clone());
    let run = |server: bool| {
        let pool = pool.clone();
        let database = database.clone();
        async move {
            if server {
                migrate(&pool).await.unwrap();
            } else {
                database.migrate().await.unwrap();
            }
        }
    };

    run(server_first).await;
    run(!server_first).await;
    let server_rows = recorded(&pool, MIGRATIONS_TABLE).await;
    let db_rows = recorded(&pool, "_sqlx_migrations").await;
    assert_eq!(server_rows.len(), shipped());
    assert_ne!(db_rows.len(), 0);
    for (version, _) in &server_rows {
        assert!(
            db_rows.iter().all(|(other, _)| other != version),
            "the two tables must be independent, {version} is in both"
        );
    }

    for _ in 0..2 {
        run(server_first).await;
        run(!server_first).await;
        assert_eq!(recorded(&pool, MIGRATIONS_TABLE).await, server_rows);
        assert_eq!(recorded(&pool, "_sqlx_migrations").await, db_rows);
    }

    let names = tables(&pool).await;
    for expected in [
        "_mtg_server_migrations",
        "_sqlx_migrations",
        "cards",
        "game_events",
        "import_runs",
        "printings",
    ] {
        assert!(
            names.iter().any(|n| n == expected),
            "{expected} in {names:?}"
        );
    }
}

#[sqlx::test(migrations = false)]
async fn the_server_then_mtg_db_migrate_one_database(pool: PgPool) {
    both_in_order(pool, true).await;
}

#[sqlx::test(migrations = false)]
async fn mtg_db_then_the_server_migrate_one_database(pool: PgPool) {
    both_in_order(pool, false).await;
}

#[sqlx::test(migrations = false)]
async fn migrating_again_keeps_the_stored_games(pool: PgPool) {
    migrate(&pool).await.unwrap();
    let store = PgEventStore::new(pool.clone());
    let log = vec![
        Event::FirstPlayerChosen {
            player: PlayerId(2),
        },
        Event::MulliganPhaseBegan,
    ];
    store.append(game_id(1), 0, &log).await.unwrap();

    Database::from_pool(pool.clone()).migrate().await.unwrap();
    migrate(&pool).await.unwrap();
    assert_eq!(store.load(game_id(1)).await.unwrap(), log);
}

#[sqlx::test(migrations = false)]
async fn both_migrators_running_at_once_do_not_conflict(pool: PgPool) {
    let runs: Vec<_> = (0..8)
        .map(|n| {
            let pool = pool.clone();
            tokio::spawn(async move {
                if n % 2 == 0 {
                    migrate(&pool).await.map_err(|e| e.to_string())
                } else {
                    Database::from_pool(pool)
                        .migrate()
                        .await
                        .map_err(|e| e.to_string())
                }
            })
        })
        .collect();
    for run in runs {
        run.await.unwrap().unwrap();
    }
    assert_eq!(recorded(&pool, MIGRATIONS_TABLE).await.len(), shipped());
    assert_ne!(recorded(&pool, "_sqlx_migrations").await.len(), 0);
}

#[sqlx::test(migrations = false)]
async fn the_server_checks_its_own_table_for_edited_migrations(pool: PgPool) {
    migrate(&pool).await.unwrap();
    sqlx::query("UPDATE _mtg_server_migrations SET checksum = '\\x00'")
        .execute(&pool)
        .await
        .unwrap();
    let refused = migrate(&pool).await.unwrap_err();
    assert!(
        matches!(refused, MigrateError::VersionMismatch(_)),
        "{refused}"
    );

    // mtg-db neither notices nor cares: its records are somewhere else.
    Database::from_pool(pool).migrate().await.unwrap();
}
