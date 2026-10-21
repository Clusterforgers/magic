//! Goal: the join code of a recorded game is stored as its SHA-256 and nothing else, it finds the game again after a restart, a game stored before codes existed has none, and the database keeps the codes of games that can still be joined apart.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use std::net::IpAddr;
use std::time::Instant;

use common::roomy;
use mtg_server::registry::SeatRequest;
use mtg_server::{GameRecords, Registry, ServerConfig, demo, migrate};
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use uuid::Uuid;

fn config() -> ServerConfig {
    ServerConfig {
        fixed_seed: Some(2),
        game: roomy(),
        ..ServerConfig::default()
    }
}

fn pair() -> Vec<SeatRequest> {
    ["mono-green", "mono-red"]
        .iter()
        .enumerate()
        .map(|(n, id)| SeatRequest {
            name: format!("Player {n}"),
            deck: demo::find(id).unwrap(),
        })
        .collect()
}

fn address() -> IpAddr {
    "192.0.2.1".parse().unwrap()
}

#[sqlx::test(migrations = false)]
async fn only_the_digest_of_the_code_is_stored_and_it_finds_the_game_after_a_restart(pool: PgPool) {
    migrate(&pool).await.unwrap();
    let registry = Registry::with_database(&config(), pool.clone());
    let created = registry.create(&pair()).await.unwrap();
    let code = created.code.expose().to_owned();

    let record = GameRecords::new(pool.clone())
        .get(created.id)
        .await
        .unwrap()
        .unwrap();
    let digest: [u8; 32] = Sha256::digest(code.as_bytes()).into();
    assert_eq!(record.code_digest, Some(digest));

    // No column of either table holds the code itself.
    for table in ["games", "seats"] {
        let hits: i64 = sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
            "SELECT count(*) FROM {table} t WHERE row_to_json(t)::text LIKE '%' || $1 || '%'"
        )))
        .bind(&code)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(hits, 0, "{table} holds the join code");
    }

    // A new process knows the code again from the digest alone.
    let again = Registry::with_database(&config(), pool.clone());
    assert_eq!(again.resume().await.unwrap().resumed, 1);
    assert_eq!(
        again.find_by_code(&code, address(), Instant::now()),
        Ok(Some(created.id))
    );
    assert_eq!(
        again.find_by_code("ABCD2345", address(), Instant::now()),
        Ok(None)
    );
}

#[sqlx::test(migrations = false)]
async fn a_game_stored_before_codes_existed_has_none_and_cannot_be_found_by_one(pool: PgPool) {
    migrate(&pool).await.unwrap();
    let game = Uuid::new_v4();
    let recipe = serde_json::json!({
        "format": "normal",
        "seed": 3,
        "decks": [{"kind": "demo", "id": "mono-green"}, {"kind": "demo", "id": "mono-red"}],
    });
    sqlx::query("INSERT INTO games (id, status, recipe) VALUES ($1, 'lobby', $2)")
        .bind(game)
        .bind(recipe)
        .execute(&pool)
        .await
        .unwrap();
    for number in 0..2_i32 {
        sqlx::query(
            "INSERT INTO seats (game_id, seat, name, token_digest) VALUES ($1, $2, 'x', $3)",
        )
        .bind(game)
        .bind(number)
        .bind(vec![7_u8; 32])
        .execute(&pool)
        .await
        .unwrap();
    }
    let registry = Registry::with_database(&config(), pool.clone());
    assert_eq!(registry.resume().await.unwrap().resumed, 1);
    // Every code of the alphabet misses it, and the game itself is there.
    assert_eq!(
        registry.find_by_code("AAAAAAAA", address(), Instant::now()),
        Ok(None)
    );
    assert!(registry.describe(game.into()).await.is_some());
}

#[sqlx::test(migrations = false)]
async fn the_database_keeps_the_codes_of_games_that_can_still_be_joined_apart(pool: PgPool) {
    migrate(&pool).await.unwrap();
    let registry = Registry::with_database(&config(), pool.clone());
    let first = registry.create(&pair()).await.unwrap();
    let second = registry.create(&pair()).await.unwrap();

    let copy = "UPDATE games SET code_digest = (SELECT code_digest FROM games WHERE id = $1) WHERE id = $2";
    let clash = sqlx::query(copy)
        .bind(first.id.uuid())
        .bind(second.id.uuid())
        .execute(&pool)
        .await;
    assert!(clash.is_err(), "two live games shared a code digest");

    // Once the first game is over its code may come up again.
    sqlx::query("UPDATE games SET status = 'finished' WHERE id = $1")
        .bind(first.id.uuid())
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query(copy)
        .bind(first.id.uuid())
        .bind(second.id.uuid())
        .execute(&pool)
        .await
        .unwrap();
}
