//! Goal: account preferences are one row per account that reads back as saved, default when absent, replaced by a later save, private per account, refused by the database when the stops are no array, and removed with the account's transaction.

#![allow(clippy::unwrap_used)]

use mtg_db::{Database, PrefsRepo, StoredPrefs};
use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;

fn user(n: u128) -> Uuid {
    Uuid::from_u128(n)
}

async fn repo(pool: PgPool) -> PrefsRepo {
    let database = Database::from_pool(pool);
    database.migrate().await.unwrap();
    database.prefs()
}

fn prefs(on: bool) -> StoredPrefs {
    StoredPrefs {
        default_stops: json!([{"side": "Own", "step": "Upkeep", "keep": true}]),
        end_turn_by_hand: on,
    }
}

#[sqlx::test(migrations = false)]
async fn absent_means_defaults_and_a_save_is_replaced_by_the_next(pool: PgPool) {
    let repo = repo(pool).await;
    assert_eq!(repo.get(user(1)).await.unwrap(), StoredPrefs::default());
    repo.put(user(1), &prefs(true)).await.unwrap();
    assert_eq!(repo.get(user(1)).await.unwrap(), prefs(true));
    repo.put(user(1), &prefs(false)).await.unwrap();
    assert_eq!(repo.get(user(1)).await.unwrap(), prefs(false));
    assert_eq!(repo.get(user(2)).await.unwrap(), StoredPrefs::default());
}

#[sqlx::test(migrations = false)]
async fn the_database_refuses_stops_that_are_no_array(pool: PgPool) {
    let repo = repo(pool).await;
    let bad = StoredPrefs {
        default_stops: json!({"side": "Own"}),
        end_turn_by_hand: false,
    };
    assert!(repo.put(user(1), &bad).await.is_err());
}

#[sqlx::test(migrations = false)]
async fn deleting_an_account_keeps_the_others(pool: PgPool) {
    let repo = repo(pool.clone()).await;
    repo.put(user(1), &prefs(true)).await.unwrap();
    repo.put(user(2), &prefs(true)).await.unwrap();
    let mut tx = pool.begin().await.unwrap();
    assert_eq!(PrefsRepo::delete_all_of(&mut tx, user(1)).await.unwrap(), 1);
    tx.commit().await.unwrap();
    assert_eq!(repo.get(user(1)).await.unwrap(), StoredPrefs::default());
    assert_eq!(repo.get(user(2)).await.unwrap(), prefs(true));
}

#[sqlx::test(migrations = false)]
async fn a_database_without_the_table_holds_no_preferences(pool: PgPool) {
    let mut tx = pool.begin().await.unwrap();
    assert_eq!(PrefsRepo::delete_all_of(&mut tx, user(1)).await.unwrap(), 0);
}
