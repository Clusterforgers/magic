//! Goal: the in-memory and the Postgres store keep to one contract: a gapless append-only log per game, appends that do not continue it refused whole, games isolated from each other, and exactly one winner when appends to the same position race.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use common::game_id;
use mtg_core::ids::PlayerId;
use mtg_engine::Event;
use mtg_server::{EventStore, MemoryStore, PgEventStore, StoreError, migrate};
use sqlx::PgPool;

/// `n` distinct events tagged with `tag`, so a test can tell whose events it is looking at.
fn events(tag: u64, n: u64) -> Vec<Event> {
    (0..n)
        .map(|i| Event::FirstPlayerChosen {
            player: PlayerId(tag * 1000 + i),
        })
        .collect()
}

async fn appends_accumulate_and_load_in_order<S: EventStore>(store: S) {
    let all = events(1, 9);
    store.append(game_id(1), 0, &all[..1]).await.unwrap();
    store.append(game_id(1), 1, &all[1..4]).await.unwrap();
    store.append(game_id(1), 4, &all[4..]).await.unwrap();
    assert_eq!(store.load(game_id(1)).await.unwrap(), all);
}

async fn an_unknown_game_loads_empty_and_empty_appends_change_nothing<S: EventStore>(store: S) {
    assert_eq!(store.load(game_id(1)).await.unwrap(), []);
    store.append(game_id(1), 0, &[]).await.unwrap();
    assert_eq!(store.load(game_id(1)).await.unwrap(), []);
    assert_eq!(
        store.append(game_id(1), 2, &[]).await,
        Err(StoreError::OutOfOrder {
            from_seq: 2,
            stored: 0
        })
    );

    store.append(game_id(1), 0, &events(1, 3)).await.unwrap();
    store.append(game_id(1), 3, &[]).await.unwrap();
    assert_eq!(store.load(game_id(1)).await.unwrap(), events(1, 3));
}

async fn appends_that_do_not_continue_the_log_are_refused_whole<S: EventStore>(store: S) {
    let kept = events(1, 3);
    store.append(game_id(1), 0, &kept).await.unwrap();
    for from_seq in [0, 1, 2, 4, 5, 1000, u64::MAX] {
        for batch in [1, 2, 5] {
            let refused = store.append(game_id(1), from_seq, &events(2, batch)).await;
            assert_eq!(
                refused,
                Err(StoreError::OutOfOrder {
                    from_seq,
                    stored: 3
                }),
                "from_seq {from_seq}, {batch} events"
            );
            assert_eq!(store.load(game_id(1)).await.unwrap(), kept);
        }
    }
    // A refused append is not a poisoned game: the right position still works.
    store.append(game_id(1), 3, &events(2, 2)).await.unwrap();
    assert_eq!(store.load(game_id(1)).await.unwrap().len(), 5);
}

async fn appending_the_same_batch_twice_is_refused_the_second_time<S: EventStore>(store: S) {
    let batch = events(1, 4);
    store.append(game_id(1), 0, &batch).await.unwrap();
    assert_eq!(
        store.append(game_id(1), 0, &batch).await,
        Err(StoreError::OutOfOrder {
            from_seq: 0,
            stored: 4
        })
    );
    assert_eq!(store.load(game_id(1)).await.unwrap(), batch);
}

async fn games_do_not_share_logs<S: EventStore>(store: S) {
    store.append(game_id(1), 0, &events(1, 3)).await.unwrap();
    // The other game starts at 0 although game 1 is already at 3.
    store.append(game_id(2), 0, &events(2, 5)).await.unwrap();
    store.append(game_id(1), 3, &events(1, 1)).await.unwrap();
    assert_eq!(
        store.append(game_id(3), 3, &events(3, 1)).await,
        Err(StoreError::OutOfOrder {
            from_seq: 3,
            stored: 0
        })
    );

    let mut first = events(1, 3);
    first.extend(events(1, 1));
    assert_eq!(store.load(game_id(1)).await.unwrap(), first);
    assert_eq!(store.load(game_id(2)).await.unwrap(), events(2, 5));
    assert_eq!(store.load(game_id(3)).await.unwrap(), []);
}

async fn racing_for_a_position_has_exactly_one_winner<S: EventStore + Clone>(store: S) {
    const RACERS: u64 = 8;
    const POSITIONS: u64 = 12;
    for position in 0..POSITIONS {
        let tasks: Vec<_> = (0..RACERS)
            .map(|racer| {
                let store = store.clone();
                tokio::spawn(async move {
                    let mine = events(racer + 1, 1);
                    store.append(game_id(1), position, &mine).await
                })
            })
            .collect();
        let mut winners = 0;
        for task in tasks {
            match task.await.unwrap() {
                Ok(()) => winners += 1,
                Err(error) => assert_eq!(
                    error,
                    StoreError::OutOfOrder {
                        from_seq: position,
                        stored: position + 1
                    }
                ),
            }
        }
        assert_eq!(winners, 1, "position {position}");
        assert_eq!(
            store.load(game_id(1)).await.unwrap().len() as u64,
            position + 1
        );
    }
}

async fn racing_batches_are_stored_whole_or_not_at_all<S: EventStore + Clone>(store: S) {
    const RACERS: u64 = 8;
    const BATCH: u64 = 5;
    let mut expected = 0;
    for round in 0..4 {
        let tasks: Vec<_> = (0..RACERS)
            .map(|racer| {
                let store = store.clone();
                tokio::spawn(async move {
                    let mine = events(round * RACERS + racer + 1, BATCH);
                    let stored = store.append(game_id(7), round * BATCH, &mine).await;
                    (racer, stored)
                })
            })
            .collect();
        let mut winner = None;
        for task in tasks {
            let (racer, stored) = task.await.unwrap();
            if stored.is_ok() {
                assert_eq!(winner.replace(racer), None, "two winners in round {round}");
            }
        }
        let winner = winner.expect("somebody wins");
        expected += BATCH;

        let log = store.load(game_id(7)).await.unwrap();
        assert_eq!(log.len() as u64, expected);
        let tail = &log[usize::try_from(round * BATCH).unwrap()..];
        assert_eq!(
            tail,
            events(round * RACERS + winner + 1, BATCH),
            "round {round}"
        );
    }
}

async fn appends_to_different_games_all_succeed_together<S: EventStore + Clone>(store: S) {
    let tasks: Vec<_> = (0..16u64)
        .map(|n| {
            let store = store.clone();
            tokio::spawn(async move {
                for chunk in 0..5 {
                    store
                        .append(game_id(u128::from(n)), chunk * 3, &events(n, 3))
                        .await
                        .unwrap();
                }
            })
        })
        .collect();
    for task in tasks {
        task.await.unwrap();
    }
    for n in 0..16u64 {
        assert_eq!(store.load(game_id(u128::from(n))).await.unwrap().len(), 15);
    }
}

/// Runs every contract case against the memory store and against a freshly migrated database.
macro_rules! contract {
    ($($case:ident),+ $(,)?) => {
        mod memory {
            use super::*;
            $(
                #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
                async fn $case() {
                    super::$case(MemoryStore::new()).await;
                }
            )+
        }

        mod postgres {
            use super::*;
            $(
                #[sqlx::test(migrations = false)]
                async fn $case(pool: PgPool) {
                    migrate(&pool).await.unwrap();
                    super::$case(PgEventStore::new(pool)).await;
                }
            )+
        }
    };
}

contract!(
    appends_accumulate_and_load_in_order,
    an_unknown_game_loads_empty_and_empty_appends_change_nothing,
    appends_that_do_not_continue_the_log_are_refused_whole,
    appending_the_same_batch_twice_is_refused_the_second_time,
    games_do_not_share_logs,
    racing_for_a_position_has_exactly_one_winner,
    racing_batches_are_stored_whole_or_not_at_all,
    appends_to_different_games_all_succeed_together,
);
