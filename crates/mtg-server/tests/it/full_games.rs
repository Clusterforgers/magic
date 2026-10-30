//! Goal: bot clients playing only through the handle API finish whole games with 2, 3 and 4 seats; every decision reaches only its seat, every client sees one gapless stream, and the stored log replays to the live state and to the same game played straight on an engine.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use common::{game_id, play_table, reference, roomy, table_setup};
use mtg_engine::replay;
use mtg_server::{EventStore, MemoryStore, spawn_game_with};
use rstest::rstest;

#[rstest]
#[tokio::test]
async fn bots_play_a_full_game_through_the_handle(
    #[values(2, 3, 4)] players: usize,
    #[values(1, 2, 3)] seed: u64,
) {
    let store = MemoryStore::new();
    let id = game_id(100 * u128::from(seed) + players as u128);
    let setup = table_setup(players, seed);
    let handle = spawn_game_with(id, setup.clone(), store.clone(), roomy()).unwrap();

    let reports = play_table(&handle, players, seed).await;

    let logged = store.load(id).await.unwrap();
    let status = handle.status().await.unwrap();
    let replayed = replay(&setup, &logged).unwrap();
    assert_eq!(status.state_hash, Some(replayed.state_hash()));
    assert_eq!(status.events, logged.len() as u64);
    assert_eq!(status.result, replayed.result());
    assert_eq!(status.waiting_on, None);

    // The same bots on a bare engine play the very same game.
    let straight = reference(players, seed);
    common::assert_same_game(&logged, &straight.events);
    assert_eq!(status.state_hash, Some(straight.final_hash));
    assert_eq!(status.result, Some(straight.result));

    assert_eq!(reports.len(), players);
    let mut answers = 0;
    let mut decisions = 0;
    for report in &reports {
        assert_eq!(report.result, straight.result);
        assert_eq!(report.events_seen, logged.len() as u64, "{:?}", report.seat);
        assert_eq!(report.snapshots, 0, "nobody needed a resync");
        assert_eq!(report.errors, []);
        answers += report.answers;
        decisions += report.decisions;
    }
    assert_eq!(answers, straight.answers);
    assert_eq!(decisions, answers, "each decision reached exactly one seat");
}
