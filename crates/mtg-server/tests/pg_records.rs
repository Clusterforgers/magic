//! Goal: creating a game writes its recipe, seats and token digests (never a token) to Postgres in one step or not at all, a game that never started is listed as a lobby, and joins, the start and the end move the records on as they happen.
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;
mod net;

use common::{Bot, drive, roomy};
use mtg_server::registry::{CreateError, SeatRequest};
use mtg_server::{
    DeckRef, GameId, GameRecipe, GameRecords, RecordStatus, Registry, SeatState, ServerConfig,
    demo, migrate,
};
use net::{TestServer, bot_config, two_seats};
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use uuid::Uuid;

fn config(seed: u64) -> ServerConfig {
    ServerConfig {
        fixed_seed: Some(seed),
        game: roomy(),
        ..ServerConfig::default()
    }
}

fn pair() -> Vec<SeatRequest> {
    ["mono-green", "mono-red"]
        .iter()
        .enumerate()
        .map(|(i, id)| SeatRequest {
            name: format!("Player {i}"),
            deck: demo::find(id).unwrap(),
        })
        .collect()
}

fn digest(token: &str) -> [u8; 32] {
    Sha256::digest(token.as_bytes()).into()
}

async fn count(pool: &PgPool, table: &str) -> i64 {
    // `table` is always one of this file's own literals.
    sqlx::query_scalar(sqlx::AssertSqlSafe(format!("SELECT count(*) FROM {table}")))
        .fetch_one(pool)
        .await
        .unwrap()
}

#[sqlx::test(migrations = false)]
async fn a_new_game_is_stored_with_its_recipe_its_seats_and_only_the_digests_of_its_tokens(
    pool: PgPool,
) {
    migrate(&pool).await.unwrap();
    let registry = Registry::with_database(&config(u64::MAX), pool.clone());
    let created = registry.create(&pair()).await.unwrap();
    let records = GameRecords::new(pool.clone());

    let record = records.get(created.id).await.unwrap().unwrap();
    assert_eq!(record.status, RecordStatus::Lobby);
    assert_eq!((record.result, record.failure), (None, None));
    assert!(record.age.as_secs() < 60);
    let recipe = GameRecipe::normal(
        u64::MAX,
        ["mono-green", "mono-red"].map(|id| DeckRef::Demo { id: id.to_owned() }),
    )
    .with_fingerprints()
    .unwrap();
    assert_eq!(
        recipe.fingerprints.len(),
        2,
        "a new game keeps its decks' fingerprints"
    );
    assert_eq!(record.recipe, serde_json::to_value(&recipe).unwrap());
    let rebuilt: GameRecipe = serde_json::from_value(record.recipe).unwrap();
    assert_eq!(rebuilt.seed, u64::MAX, "a seed past 2^63 survives jsonb");

    assert_eq!(record.seats.len(), 2);
    for (seat, (stored, (number, token))) in record.seats.iter().zip(&created.seats).enumerate() {
        assert_eq!(stored.seat, seat);
        assert_eq!(number.index(), seat);
        assert_eq!(stored.name, format!("Player {seat}"));
        assert_eq!(stored.token_digest, Some(digest(token.expose())));
        assert_eq!(stored.state, SeatState::Issued);
        assert!(!stored.joined);
    }
    assert_ne!(record.seats[0].token_digest, record.seats[1].token_digest);

    // No column of either table holds a token in clear.
    for (_, token) in &created.seats {
        for table in ["games", "seats"] {
            let hits: i64 = sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
                "SELECT count(*) FROM {table} t WHERE row_to_json(t)::text LIKE '%' || $1 || '%'"
            )))
            .bind(token.expose())
            .fetch_one(&pool)
            .await
            .unwrap();
            assert_eq!(hits, 0, "{table} holds a token");
        }
    }
}

#[sqlx::test(migrations = false)]
async fn games_that_never_started_are_listed_as_lobbies_oldest_first(pool: PgPool) {
    migrate(&pool).await.unwrap();
    let registry = Registry::with_database(&config(1), pool.clone());
    let mut ids = Vec::new();
    for _ in 0..3 {
        ids.push(registry.create(&pair()).await.unwrap().id);
        tokio::time::sleep(std::time::Duration::from_millis(5)).await;
    }

    let listed = GameRecords::new(pool).unfinished().await.unwrap();
    assert_eq!(listed.iter().map(|r| r.id).collect::<Vec<_>>(), ids);
    assert!(listed.iter().all(|r| r.status == RecordStatus::Lobby));
    assert!(listed.iter().all(|r| r.seats.len() == 2));
}

#[sqlx::test(migrations = false)]
async fn joins_the_start_and_the_end_update_the_records_as_they_happen(pool: PgPool) {
    migrate(&pool).await.unwrap();
    let seed = 4;
    let registry = Registry::with_database(&config(seed), pool.clone());
    let created = registry.create(&pair()).await.unwrap();
    let handle = registry.handle(created.id).unwrap();
    let records = GameRecords::new(pool.clone());
    let flags = |record: &mtg_server::GameRecord| -> Vec<bool> {
        record.seats.iter().map(|s| s.joined).collect()
    };
    let now = || async { records.get(created.id).await.unwrap().unwrap() };

    let mut bots = vec![Bot::join(&handle, 0, seed).await];
    handle.status().await.unwrap(); // the actor has handled the join, record included
    let record = now().await;
    assert_eq!(flags(&record), [true, false]);
    assert_eq!(record.status, RecordStatus::Lobby);

    bots.push(Bot::join(&handle, 1, seed).await);
    handle.status().await.unwrap();
    assert_eq!(flags(&now().await), [true, true]);

    for bot in &bots {
        handle
            .send(bot.seat, mtg_protocol::ClientMessage::Ready)
            .await
            .unwrap();
    }
    assert!(handle.status().await.unwrap().started);
    assert_eq!(now().await.status, RecordStatus::Running);
    let started: bool = sqlx::query_scalar("SELECT started_at IS NOT NULL FROM games")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert!(started);

    let result = drive(&mut bots, &handle, usize::MAX).await.unwrap();
    // The record is written before anyone hears `GameOver`.
    let record = now().await;
    assert_eq!(record.status, RecordStatus::Finished);
    assert_eq!(record.result, Some(result));
    let finished: bool = sqlx::query_scalar("SELECT finished_at IS NOT NULL FROM games")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert!(finished);
    assert_eq!(records.unfinished().await.unwrap(), []);
}

#[sqlx::test(migrations = false)]
async fn a_creation_that_fails_leaves_no_records_behind(pool: PgPool) {
    migrate(&pool).await.unwrap();
    let registry = Registry::with_database(
        &ServerConfig {
            max_games: 1,
            ..config(1)
        },
        pool.clone(),
    );
    registry.create(&pair()).await.unwrap();

    let full = registry.create(&pair()).await.unwrap_err();
    assert_eq!(full, CreateError::Full);
    let one_seat = pair().into_iter().take(1).collect::<Vec<_>>();
    assert!(matches!(
        registry.create(&one_seat).await.unwrap_err(),
        CreateError::Recipe(_)
    ));
    assert_eq!(
        (count(&pool, "games").await, count(&pool, "seats").await),
        (1, 2)
    );
    assert_eq!(registry.len(), 1);
}

#[sqlx::test(migrations = false)]
async fn a_game_the_database_cannot_record_is_not_created(pool: PgPool) {
    migrate(&pool).await.unwrap();
    let registry = Registry::with_database(&config(1), pool.clone());
    pool.close().await;

    let error = registry.create(&pair()).await.unwrap_err();
    assert!(matches!(error, CreateError::Record(_)), "{error:?}");
    assert!(registry.is_empty());
}

#[sqlx::test(migrations = false)]
async fn the_rest_path_answers_503_when_the_game_cannot_be_recorded(pool: PgPool) {
    migrate(&pool).await.unwrap();
    let server = TestServer::start_on(pool.clone(), bot_config()).await;
    let ok = net::post_json(server.addr, "/api/games", &two_seats().to_string()).await;
    assert_eq!(ok.status, 201, "{}", ok.text());
    assert_eq!(count(&pool, "games").await, 1);

    pool.close().await;
    let refused = net::post_json(server.addr, "/api/games", &two_seats().to_string()).await;
    assert_eq!(refused.status, 503);
    assert_eq!(refused.error_code(), "unavailable");
    assert!(!refused.text().contains("sqlx"), "details stay in the log");
    assert_eq!(server.registry.len(), 1);
}

#[sqlx::test(migrations = false)]
async fn a_game_built_by_hand_has_no_recipe_and_is_not_recorded(pool: PgPool) {
    migrate(&pool).await.unwrap();
    let registry = Registry::with_database(&config(1), pool.clone());
    let setup = common::table_setup(2, 1);
    let created = registry
        .register(setup, vec!["A".to_owned(), "B".to_owned()])
        .await
        .unwrap();

    assert!(registry.handle(created.id).is_some());
    assert_eq!(count(&pool, "games").await, 0);
    let unknown = GameId(Uuid::from_u128(7));
    assert_eq!(GameRecords::new(pool).get(unknown).await.unwrap(), None);
}
