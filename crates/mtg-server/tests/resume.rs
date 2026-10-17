//! Goal: a game rebuilt from its stored log (the server restarting) picks up exactly where it stopped, waits for its players, and finishes in the very state an uninterrupted game reaches; logs that cannot be resumed are reported, not guessed at.
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use common::{Bot, Client, drive, game_id, reference, roomy, seat, table_setup};
use mtg_engine::{Event, replay};
use mtg_protocol::{ClientMessage, ServerMessage};
use mtg_server::{EventStore, MemoryStore, StartError, resume_game_with, spawn_game_with};
use rstest::rstest;

#[rstest]
#[tokio::test]
async fn a_resumed_game_continues_identically(
    #[values(2, 3, 4)] players: usize,
    #[values(0, 1, 25, 200)] answers_before: usize,
) {
    let seed = 17;
    let id = game_id(1);
    let setup = table_setup(players, seed);
    let store = MemoryStore::new();
    let first = spawn_game_with(id, setup.clone(), store.clone(), roomy()).unwrap();
    let mut bots = Vec::new();
    for n in 0..players {
        bots.push(Bot::join(&first, n, seed).await);
    }
    for bot in &bots {
        first.send(bot.seat, ClientMessage::Ready).await.unwrap();
    }
    assert_eq!(drive(&mut bots, &first, answers_before).await, None);
    assert_eq!(
        bots.iter().map(|b| b.answers).sum::<usize>(),
        answers_before
    );
    let stopped = first.status().await.unwrap();
    assert_eq!(stopped.events, store.load(id).await.unwrap().len() as u64);

    // The process dies: the actor goes away with its last handle, the store survives.
    drop(first);

    let second = resume_game_with(store.clone(), id, setup.clone(), roomy())
        .await
        .unwrap();
    let resumed = second.status().await.unwrap();
    assert!(resumed.started);
    assert_eq!(resumed.events, stopped.events);
    assert_eq!(resumed.state_hash, stopped.state_hash);
    assert_eq!(
        resumed.waiting_on, stopped.waiting_on,
        "it waits for the same player"
    );
    assert_eq!(resumed.connected, [], "and for everyone to come back");

    for bot in &mut bots {
        bot.rejoin(&second).await;
    }
    let result = drive(&mut bots, &second, usize::MAX).await;

    let straight = reference(players, seed);
    assert_eq!(result, Some(straight.result));
    let logged = store.load(id).await.unwrap();
    assert_eq!(logged, straight.events);
    let status = second.status().await.unwrap();
    assert_eq!(status.state_hash, Some(straight.final_hash));
    assert_eq!(
        status.state_hash,
        Some(replay(&setup, &logged).unwrap().state_hash())
    );
    for bot in &bots {
        assert_eq!(bot.snapshots, 1, "{:?} was resynced once", bot.seat);
        assert_eq!(bot.next_seq, logged.len() as u64);
        assert_eq!(bot.errors, []);
    }
}

#[tokio::test]
async fn a_finished_game_resumes_as_finished() {
    let seed = 3;
    let id = game_id(2);
    let setup = table_setup(2, seed);
    let store = MemoryStore::new();
    let first = spawn_game_with(id, setup.clone(), store.clone(), roomy()).unwrap();
    let reports = common::play_table(&first, 2, seed).await;
    let result = reports[0].result;
    drop(first);

    let second = resume_game_with(store, id, setup, roomy()).await.unwrap();
    let status = second.status().await.unwrap();
    assert_eq!(status.result, Some(result));
    assert_eq!(status.waiting_on, None);

    let mut client = Client::join(&second, 0).await;
    assert!(matches!(client.next().await, ServerMessage::Welcome { .. }));
    assert!(matches!(
        client.next().await,
        ServerMessage::Snapshot { .. }
    ));
    assert_eq!(client.next().await, ServerMessage::GameOver { result });
}

#[tokio::test]
async fn a_game_with_nothing_stored_returns_to_its_lobby() {
    let store = MemoryStore::new();
    let handle = resume_game_with(store, game_id(3), table_setup(2, 1), roomy())
        .await
        .unwrap();
    let status = handle.status().await.unwrap();
    assert!(!status.started);
    assert_eq!(status.events, 0);
}

#[tokio::test]
async fn events_the_store_missed_are_caught_up_before_play_resumes() {
    let seed = 5;
    let id = game_id(4);
    let straight = reference(3, seed);
    // A log that ends after the opening's first internal batch (first player, three shuffles, the
    // start of the mulligans) but before the hands are dealt: the engine has to run on by itself.
    let cut = 3 + 2;
    let store = MemoryStore::new();
    store.append(id, 0, &straight.events[..cut]).await.unwrap();

    let handle = resume_game_with(store.clone(), id, table_setup(3, seed), roomy())
        .await
        .unwrap();
    let status = handle.status().await.unwrap();
    assert!(status.events > cut as u64);
    let logged = store.load(id).await.unwrap();
    assert_eq!(logged.len() as u64, status.events);
    assert_eq!(
        logged[..],
        straight.events[..logged.len()],
        "the same opening, now complete"
    );
    assert!(status.waiting_on.is_some());
}

#[tokio::test]
async fn a_log_that_does_not_replay_is_reported() {
    let id = game_id(5);
    let store = MemoryStore::new();
    let nonsense = [Event::PriorityPassed { player: seat(0) }];
    store.append(id, 0, &nonsense).await.unwrap();
    let refused = resume_game_with(store, id, table_setup(2, 1), roomy()).await;
    assert!(matches!(refused, Err(StartError::Engine(_))));
}

#[tokio::test]
async fn a_failing_store_is_reported_when_resuming() {
    let store = common::FlakyStore::new();
    store.fail(true);
    let id = game_id(6);
    let straight = reference(2, 9);
    store
        .inner()
        .append(id, 0, &straight.events[..2])
        .await
        .unwrap();
    // Catching up needs a write, which fails.
    let refused = resume_game_with(store, id, table_setup(2, 9), roomy()).await;
    assert!(matches!(refused, Err(StartError::Store(_))));
}
