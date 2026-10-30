//! Goal: a game actor on the Postgres store plays exactly the game it plays on the in-memory one, and a game rebuilt from rows after a restart continues to the same end.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use common::{Bot, drive, game_id, play_table, reference, roomy, table_setup};
use mtg_engine::replay;
use mtg_protocol::ClientMessage;
use mtg_server::{EventStore, PgEventStore, migrate, resume_game_with, spawn_game_with};
use sqlx::PgPool;
use sqlx::postgres::{PgConnectOptions, PgPoolOptions};

#[sqlx::test(migrations = false)]
async fn bots_play_full_games_on_the_postgres_store(pool: PgPool) {
    migrate(&pool).await.unwrap();
    for (n, (players, seed)) in [(2, 1), (3, 2), (4, 3)].into_iter().enumerate() {
        let id = game_id(n as u128 + 1);
        let setup = table_setup(players, seed);
        let store = PgEventStore::new(pool.clone());
        let handle = spawn_game_with(id, setup.clone(), store.clone(), roomy()).unwrap();

        let reports = play_table(&handle, players, seed).await;

        let straight = reference(players, seed);
        let logged = store.load(id).await.unwrap();
        common::assert_same_game(&logged, &straight.events);
        let status = handle.status().await.unwrap();
        assert_eq!(status.state_hash, Some(straight.final_hash));
        assert_eq!(status.events, logged.len() as u64);
        assert_eq!(
            replay(&setup, &logged).unwrap().state_hash(),
            straight.final_hash
        );
        for report in &reports {
            assert_eq!(report.result, straight.result);
            assert_eq!(report.errors, []);
        }
    }
}

#[sqlx::test(migrations = false)]
async fn a_game_resumes_from_its_rows_after_a_restart(
    pool_options: PgPoolOptions,
    options: PgConnectOptions,
) {
    let pool = pool_options.connect_with(options.clone()).await.unwrap();
    migrate(&pool).await.unwrap();
    let (players, seed, answers_before) = (3, 17, 150);
    let id = game_id(1);
    let setup = table_setup(players, seed);

    let first =
        spawn_game_with(id, setup.clone(), PgEventStore::new(pool.clone()), roomy()).unwrap();
    let mut bots = Vec::new();
    for n in 0..players {
        bots.push(Bot::join(&first, n, seed).await);
    }
    for bot in &bots {
        first.send(bot.seat, ClientMessage::Ready).await.unwrap();
    }
    assert_eq!(drive(&mut bots, &first, answers_before).await, None);
    let stopped = first.status().await.unwrap();

    // The process dies: the actor and its pool go away, the rows stay.
    drop(first);
    pool.close().await;

    let reborn = PgPoolOptions::new().connect_with(options).await.unwrap();
    let store = PgEventStore::new(reborn);
    let second = resume_game_with(store.clone(), id, setup.clone(), roomy())
        .await
        .unwrap();
    let resumed = second.status().await.unwrap();
    assert_eq!(resumed.events, stopped.events);
    assert_eq!(resumed.state_hash, stopped.state_hash);

    for bot in &mut bots {
        bot.rejoin(&second).await;
    }
    let result = drive(&mut bots, &second, usize::MAX).await;

    let straight = reference(players, seed);
    assert_eq!(result, Some(straight.result));
    common::assert_same_game(&store.load(id).await.unwrap(), &straight.events);
    assert_eq!(
        second.status().await.unwrap().state_hash,
        Some(straight.final_hash)
    );
}
