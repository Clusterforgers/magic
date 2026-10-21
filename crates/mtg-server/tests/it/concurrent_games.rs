//! Goal: many games share one runtime and one store without interfering: thirty concurrent games, mixed seat counts, each finishes with exactly the log and result of its own reference game.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use common::{game_id, play_table, reference, roomy, table_setup};
use mtg_engine::replay;
use mtg_server::{EventStore, MemoryStore, spawn_game_with};

const GAMES: u64 = 30;

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn thirty_games_run_side_by_side_and_each_matches_its_reference() {
    let store = MemoryStore::new();
    let mut tasks = Vec::new();
    for n in 0..GAMES {
        let store = store.clone();
        tasks.push(tokio::spawn(async move {
            let players = 2 + usize::try_from(n % 3).unwrap();
            let seed = 1_000 + n;
            let id = game_id(u128::from(n));
            let handle = spawn_game_with(id, table_setup(players, seed), store, roomy()).unwrap();
            let reports = play_table(&handle, players, seed).await;
            (id, players, seed, reports, handle.status().await.unwrap())
        }));
    }

    let mut finished = 0;
    for task in tasks {
        let (id, players, seed, reports, status) = task.await.unwrap();
        let straight = reference(players, seed);
        let logged = store.load(id).await.unwrap();
        assert_eq!(logged, straight.events, "game {id} (seed {seed}) diverged");
        assert_eq!(status.state_hash, Some(straight.final_hash));
        assert_eq!(status.result, Some(straight.result));
        assert_eq!(
            status.state_hash,
            Some(
                replay(&table_setup(players, seed), &logged)
                    .unwrap()
                    .state_hash()
            )
        );
        assert_eq!(reports.len(), players);
        for report in &reports {
            assert_eq!(report.result, straight.result);
            assert_eq!(report.events_seen, logged.len() as u64);
            assert_eq!(report.errors, []);
        }
        finished += 1;
    }
    assert_eq!(finished, GAMES);
}
