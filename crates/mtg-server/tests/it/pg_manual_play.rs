//! Goal: the flag that says a table plays with manual cards is stored with the game and kept by every restart: a game stored before the column existed reads false in every status, a started game resumes with the flag its record holds whichever way it is set, a lobby judges the flag by its roster again, and migrating a database that holds a started game leaves the game running with the flag false.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common::seat;

use mtg_protocol::ClientMessage;
use mtg_server::registry::SeatRequest;
use mtg_server::{GameId, GameRecords, MIGRATIONS_TABLE, Registry, ServerConfig, demo};
use mtg_server::{migrate, migrator};
use sqlx::PgPool;
use uuid::Uuid;

/// The last server migration before the flag.
const BEFORE_THE_FLAG: i64 = 20_261_021_000_009;
/// The migration that adds the flag.
const THE_FLAG: i64 = 20_261_023_000_012;

fn requests() -> Vec<SeatRequest> {
    ["mono-green", "mono-red"]
        .iter()
        .enumerate()
        .map(|(n, id)| SeatRequest {
            name: format!("Bot {n}"),
            deck: demo::find(id).unwrap(),
        })
        .collect()
}

/// A process on the database: a fresh registry that resumes what the last one left.
async fn boot(pool: &PgPool) -> Registry {
    let registry = Registry::with_database(&ServerConfig::default(), pool.clone());
    registry.resume().await.unwrap();
    registry
}

/// A two-seat game of built-in decks whose seats are both ready, so it runs.
async fn started(registry: &Registry) -> GameId {
    let created = registry.create(&requests()).await.unwrap();
    let handle = registry.handle(created.id).unwrap();
    for number in 0..2 {
        handle
            .send(seat(number), ClientMessage::Ready)
            .await
            .unwrap();
    }
    assert!(handle.status().await.unwrap().started);
    created.id
}

async fn flag_of(registry: &Registry, game: GameId) -> bool {
    registry
        .describe(game)
        .await
        .expect("the game is there")
        .manual_play
}

async fn stored(pool: &PgPool, game: GameId) -> bool {
    GameRecords::new(pool.clone())
        .get(game)
        .await
        .unwrap()
        .expect("the record is there")
        .manual_play
}

async fn set_stored(pool: &PgPool, game: GameId, flag: bool) {
    sqlx::query("UPDATE games SET manual_play = $2 WHERE id = $1")
        .bind(Uuid::parse_str(&game.to_string()).unwrap())
        .bind(flag)
        .execute(pool)
        .await
        .unwrap();
}

#[sqlx::test(migrations = false)]
async fn a_game_stored_before_the_flag_existed_reads_false_in_every_status(pool: PgPool) {
    migrator().run_to(BEFORE_THE_FLAG, &pool).await.unwrap();
    let mut games = Vec::new();
    for status in ["lobby", "running", "finished", "expired", "failed"] {
        let id = Uuid::new_v4();
        sqlx::query("INSERT INTO games (id, status, recipe) VALUES ($1, $2, '{}'::jsonb)")
            .bind(id)
            .bind(status)
            .execute(&pool)
            .await
            .unwrap();
        games.push((id.to_string().parse::<GameId>().unwrap(), status));
    }
    migrate(&pool).await.unwrap();

    let records = GameRecords::new(pool.clone());
    for (game, status) in &games {
        let record = records.get(*game).await.unwrap().unwrap();
        assert!(!record.manual_play, "a {status} game");
    }
    let open = records.unfinished().await.unwrap();
    assert_eq!(open.len(), 2, "the lobby and the running game");
    assert!(open.iter().all(|record| !record.manual_play));
}

#[sqlx::test(migrations = false)]
async fn a_started_game_resumes_with_the_flag_its_record_holds(pool: PgPool) {
    migrate(&pool).await.unwrap();
    let first = Registry::with_database(&ServerConfig::default(), pool.clone());
    let (plain, by_hand) = (started(&first).await, started(&first).await);
    assert!(!flag_of(&first, plain).await && !flag_of(&first, by_hand).await);
    drop(first);
    set_stored(&pool, by_hand, true).await;

    for restart in 1..=2 {
        let registry = boot(&pool).await;
        assert!(!flag_of(&registry, plain).await, "restart {restart}");
        assert!(flag_of(&registry, by_hand).await, "restart {restart}");
        assert!(!stored(&pool, plain).await && stored(&pool, by_hand).await);
    }

    // The record is the truth: the flag follows it when it is cleared again.
    set_stored(&pool, by_hand, false).await;
    let registry = boot(&pool).await;
    assert!(!flag_of(&registry, by_hand).await);
}

#[sqlx::test(migrations = false)]
async fn a_lobby_judges_the_flag_by_its_roster_after_a_restart(pool: PgPool) {
    migrate(&pool).await.unwrap();
    let first = Registry::with_database(&ServerConfig::default(), pool.clone());
    let lobby = first.create(&requests()).await.unwrap().id;
    drop(first);
    // Only a start writes the flag, so this stands for a record left by a start that did not finish.
    set_stored(&pool, lobby, true).await;

    let registry = boot(&pool).await;
    let handle = registry.handle(lobby).unwrap();
    assert!(!handle.status().await.unwrap().started);
    assert!(
        !flag_of(&registry, lobby).await,
        "no seat plays a deck by hand"
    );
}

#[sqlx::test(migrations = false)]
async fn migrating_a_database_with_a_started_game_keeps_the_game_and_reads_false(pool: PgPool) {
    migrate(&pool).await.unwrap();
    let first = Registry::with_database(&ServerConfig::default(), pool.clone());
    let game = started(&first).await;
    let hash = first
        .handle(game)
        .unwrap()
        .status()
        .await
        .unwrap()
        .state_hash;
    drop(first);

    // The database as it was before the migration ran: no column, and the migration not recorded.
    sqlx::query("ALTER TABLE games DROP COLUMN manual_play")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query(sqlx::AssertSqlSafe(format!(
        "DELETE FROM {MIGRATIONS_TABLE} WHERE version = {THE_FLAG}"
    )))
    .execute(&pool)
    .await
    .unwrap();
    migrate(&pool).await.unwrap();

    assert!(!stored(&pool, game).await);
    let registry = boot(&pool).await;
    let status = registry.handle(game).unwrap().status().await.unwrap();
    assert!(status.started);
    assert_eq!(status.state_hash, hash);
    assert!(!flag_of(&registry, game).await);
}
