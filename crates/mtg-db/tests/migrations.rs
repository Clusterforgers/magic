//! Goal: migrations apply cleanly to an empty database, are safe to re-run and to run concurrently, and `Database::connect` works from a plain URL.

mod common;

use mtg_db::{CardSearch, Database, DbError, Page};
use sqlx::ConnectOptions;
use sqlx::PgPool;
use sqlx::postgres::{PgConnectOptions, PgPoolOptions};

use common::db;

const MIGRATION_COUNT: i64 = 5;

async fn applied_migrations(pool: &PgPool) -> i64 {
    sqlx::query_scalar("SELECT count(*) FROM _sqlx_migrations WHERE success")
        .fetch_one(pool)
        .await
        .unwrap()
}

#[sqlx::test(migrations = false)]
async fn applies_to_an_empty_database_and_can_be_rerun(pool: PgPool) {
    let database = db(pool.clone());
    for _ in 0..3 {
        database.migrate().await.unwrap();
        assert_eq!(applied_migrations(&pool).await, MIGRATION_COUNT);
    }

    let tables: Vec<String> = sqlx::query_scalar(
        "SELECT table_name::text FROM information_schema.tables
         WHERE table_schema = 'public' AND table_name <> '_sqlx_migrations' ORDER BY 1",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(
        tables,
        [
            "cards",
            "import_runs",
            "precon_deck_cards",
            "precon_decks",
            "printings",
            "rulings"
        ]
    );
    let trigram: bool =
        sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM pg_extension WHERE extname = 'pg_trgm')")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(trigram);
}

#[sqlx::test(migrations = false)]
async fn rerunning_migrations_keeps_existing_data(pool: PgPool) {
    let database = db(pool.clone());
    database.migrate().await.unwrap();
    database
        .importer()
        .import_cards(common::card_stream())
        .await
        .unwrap();
    let before = common::row_counts(&pool).await;
    assert!(before.0 > 0);
    database.migrate().await.unwrap();
    assert_eq!(common::row_counts(&pool).await, before);
}

#[sqlx::test(migrations = false)]
async fn concurrent_migration_runs_do_not_conflict(pool: PgPool) {
    let runs: Vec<_> = (0..4)
        .map(|_| {
            let database = db(pool.clone());
            tokio::spawn(async move { database.migrate().await })
        })
        .collect();
    for run in runs {
        run.await.unwrap().unwrap();
    }
    assert_eq!(applied_migrations(&pool).await, MIGRATION_COUNT);
}

#[sqlx::test(migrations = false)]
async fn connect_works_from_a_plain_url_and_rejects_a_bad_one(
    _pool_options: PgPoolOptions,
    options: PgConnectOptions,
) {
    let database = Database::connect(options.to_url_lossy().as_str())
        .await
        .unwrap();
    database.migrate().await.unwrap();
    let page = database
        .cards()
        .search(&CardSearch::default(), Page::default())
        .await
        .unwrap();
    assert_eq!(page.total, 0);

    for malformed in ["not a url", "mysql://localhost/db", ""] {
        let error = Database::connect(malformed).await.unwrap_err();
        assert!(matches!(error, DbError::Query(_)), "{malformed:?}: {error}");
    }
}
