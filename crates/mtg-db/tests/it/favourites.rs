//! Goal: favourite precons are one row per account and code (starring again changes nothing), each account sees and removes only its own, the newest star comes first, a code of another shape is refused, the database refuses one too, and deleting an account's favourites in a transaction leaves the others'.

#![allow(clippy::unwrap_used)]

use mtg_db::{Database, FavouritePreconRepo, MAX_CODE_CHARS};
use sqlx::PgPool;
use uuid::Uuid;

fn user(n: u128) -> Uuid {
    Uuid::from_u128(n)
}

async fn repo(pool: PgPool) -> FavouritePreconRepo {
    let database = Database::from_pool(pool);
    database.migrate().await.unwrap();
    database.favourite_precons()
}

#[sqlx::test(migrations = false)]
async fn starring_twice_keeps_one_row_and_its_place_in_the_order(pool: PgPool) {
    let repo = repo(pool.clone()).await;
    for code in ["c18-draconic-domination", "c19-merciless-rage"] {
        assert!(repo.add(user(1), code).await.unwrap());
    }
    assert!(repo.add(user(1), "c18-draconic-domination").await.unwrap());
    assert_eq!(
        repo.list(user(1)).await.unwrap(),
        ["c19-merciless-rage", "c18-draconic-domination"]
    );
    let rows: i64 = sqlx::query_scalar("SELECT count(*) FROM favourite_precons")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(rows, 2);
}

#[sqlx::test(migrations = false)]
async fn an_account_sees_and_removes_only_its_own_stars(pool: PgPool) {
    let repo = repo(pool).await;
    repo.add(user(1), "c18-draconic-domination").await.unwrap();
    repo.add(user(2), "c18-draconic-domination").await.unwrap();
    repo.add(user(2), "c19-merciless-rage").await.unwrap();

    assert!(
        !repo.remove(user(1), "c19-merciless-rage").await.unwrap(),
        "user 1 never starred it"
    );
    assert_eq!(repo.list(user(2)).await.unwrap().len(), 2);
    assert!(
        repo.remove(user(1), "c18-draconic-domination")
            .await
            .unwrap()
    );
    assert!(
        !repo
            .remove(user(1), "c18-draconic-domination")
            .await
            .unwrap()
    );
    assert_eq!(repo.list(user(1)).await.unwrap(), Vec::<String>::new());
    assert_eq!(
        repo.list(user(2)).await.unwrap(),
        ["c19-merciless-rage", "c18-draconic-domination"]
    );
}

#[sqlx::test(migrations = false)]
async fn a_code_of_another_shape_is_refused_by_the_repo_and_by_the_table(pool: PgPool) {
    let repo = repo(pool.clone()).await;
    let long = "a".repeat(MAX_CODE_CHARS + 1);
    for code in ["", "Upper", "precon:c18", "-lead", long.as_str()] {
        assert!(!repo.add(user(1), code).await.unwrap(), "{code}");
    }
    assert_eq!(repo.list(user(1)).await.unwrap(), Vec::<String>::new());
    let direct =
        sqlx::query("INSERT INTO favourite_precons (user_id, code) VALUES ($1, 'Bad Code')")
            .bind(user(1))
            .execute(&pool)
            .await;
    assert!(direct.is_err());
}

#[sqlx::test(migrations = false)]
async fn deleting_an_account_s_favourites_leaves_the_others(pool: PgPool) {
    let repo = repo(pool.clone()).await;
    repo.add(user(1), "c18-draconic-domination").await.unwrap();
    repo.add(user(1), "c19-merciless-rage").await.unwrap();
    repo.add(user(2), "c19-merciless-rage").await.unwrap();

    let mut tx = pool.begin().await.unwrap();
    let gone = FavouritePreconRepo::delete_all_of(&mut tx, user(1))
        .await
        .unwrap();
    tx.commit().await.unwrap();

    assert_eq!(gone, 2);
    assert_eq!(repo.list(user(1)).await.unwrap(), Vec::<String>::new());
    assert_eq!(repo.list(user(2)).await.unwrap(), ["c19-merciless-rage"]);
}

#[sqlx::test(migrations = false)]
async fn a_database_without_the_table_holds_no_favourites(pool: PgPool) {
    let mut tx = pool.begin().await.unwrap();
    let gone = FavouritePreconRepo::delete_all_of(&mut tx, user(1))
        .await
        .unwrap();
    assert_eq!(gone, 0);
}
