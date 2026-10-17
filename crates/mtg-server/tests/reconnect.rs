//! Goal: a client that drops and comes back, holder or spectator, is resynced with a snapshot and its pending decision and the game continues to exactly the final state of an uninterrupted game; a replaced or too-slow connection is closed without ever blocking the game.
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use common::{
    Bot, Client, drive, game_id, passive_until_priority, reference, roomy, seat, silent,
    start_table, table_setup,
};
use mtg_engine::replay;
use mtg_protocol::{ClientMessage, ServerMessage};
use mtg_server::{EventStore, GameConfig, MIN_QUEUE, MemoryStore, spawn_game_with};
use rstest::rstest;
use tokio::sync::mpsc::error::TryRecvError;

#[derive(Debug, Clone, Copy)]
enum Dropped {
    /// The seat the game is waiting for.
    Holder,
    /// A seat the game is not waiting for.
    Spectator,
}

#[rstest]
#[tokio::test]
async fn a_client_that_drops_mid_game_resyncs_and_the_game_ends_exactly_as_if_nothing_happened(
    #[values(2, 3, 4)] players: usize,
    #[values(Dropped::Holder, Dropped::Spectator)] dropped: Dropped,
    #[values(0, 3, 40, 300)] answers_before: usize,
) {
    let seed = 7;
    let store = MemoryStore::new();
    let setup = table_setup(players, seed);
    let handle = spawn_game_with(game_id(1), setup.clone(), store.clone(), roomy()).unwrap();
    let mut bots = Vec::new();
    for n in 0..players {
        bots.push(Bot::join(&handle, n, seed).await);
    }
    for bot in &bots {
        handle.send(bot.seat, ClientMessage::Ready).await.unwrap();
    }
    assert_eq!(drive(&mut bots, &handle, answers_before).await, None);
    assert_eq!(
        bots.iter().map(|b| b.answers).sum::<usize>(),
        answers_before
    );

    // The game now waits for one seat. Cut a connection and look at what the table does meanwhile.
    let waiting = handle.status().await.unwrap().waiting_on.unwrap().index();
    let victim = match dropped {
        Dropped::Holder => waiting,
        Dropped::Spectator => (waiting + 1) % players,
    };
    handle.leave(seat(victim)).await;
    let frozen = handle.status().await.unwrap();
    assert!(!frozen.connected.contains(&seat(victim)));
    assert_eq!(frozen.waiting_on, Some(seat(waiting)));
    // Everyone else keeps answering whatever they are asked; if the game skipped the missing
    // seat they would move it along.
    assert_eq!(drive(&mut bots, &handle, usize::MAX).await, None);
    assert_eq!(
        handle.status().await.unwrap(),
        frozen,
        "nothing moves without the missing player"
    );

    // Back again: the new queue starts with a welcome, a snapshot and, for the holder, its decision.
    bots[victim].rejoin(&handle).await;
    if victim != waiting {
        // The seat the game waits for lost nothing, but its bot is holding an answer; a resync hands it the decision again.
        handle
            .send(seat(waiting), ClientMessage::Resync)
            .await
            .unwrap();
    }
    let result = drive(&mut bots, &handle, usize::MAX).await;
    assert_eq!(bots[victim].snapshots, 1);

    let straight = reference(players, seed);
    assert_eq!(result, Some(straight.result));
    let logged = store.load(game_id(1)).await.unwrap();
    assert_eq!(logged, straight.events);
    let status = handle.status().await.unwrap();
    assert_eq!(status.state_hash, Some(straight.final_hash));
    assert_eq!(
        status.state_hash,
        Some(replay(&setup, &logged).unwrap().state_hash())
    );
    for bot in &bots {
        assert_eq!(
            bot.next_seq,
            logged.len() as u64,
            "{:?} caught up",
            bot.seat
        );
        assert_eq!(bot.errors, []);
    }
}

#[tokio::test]
async fn joining_a_taken_seat_replaces_the_old_connection() {
    let store = MemoryStore::new();
    let handle = spawn_game_with(game_id(1), table_setup(2, 3), store, roomy()).unwrap();
    let mut old = Client::join(&handle, 0).await;
    let mut new = Client::join(&handle, 0).await;

    assert!(matches!(
        old.inbox.try_recv(),
        Ok(ServerMessage::Welcome { .. })
    ));
    assert_eq!(
        old.inbox.try_recv().unwrap_err(),
        TryRecvError::Disconnected
    );
    assert!(matches!(
        new.inbox.try_recv(),
        Ok(ServerMessage::Welcome { .. })
    ));

    let line = ClientMessage::Chat {
        text: "hello".to_owned(),
    };
    handle.send(seat(0), line).await.unwrap();
    handle.status().await.unwrap();
    assert!(matches!(
        new.drain().as_slice(),
        [ServerMessage::Chat { .. }]
    ));
    assert_eq!(handle.status().await.unwrap().connected, [seat(0)]);
}

#[tokio::test]
async fn a_joining_seat_gets_welcome_then_snapshot_then_its_pending_decision() {
    let store = MemoryStore::new();
    let handle = spawn_game_with(game_id(1), table_setup(3, 3), store, roomy()).unwrap();
    let (mut clients, _) = start_table(&handle, 3).await;
    let (holder, id, decision) = passive_until_priority(&handle, &mut clients).await;
    let status = handle.status().await.unwrap();

    for n in 0..3 {
        handle.leave(seat(n)).await;
        let mut back = Client::join(&handle, n).await;
        assert!(
            matches!(back.next().await, ServerMessage::Welcome { player, .. } if player == seat(n))
        );
        let ServerMessage::Snapshot { seq, state } = back.next().await else {
            panic!("the snapshot comes right after the welcome");
        };
        assert_eq!(seq, status.events);
        assert_eq!(state.viewer, seat(n));
        if n == holder {
            assert_eq!(
                back.next().await,
                ServerMessage::Decision {
                    id,
                    decision: decision.clone()
                }
            );
        }
        silent(&back.drain());
    }
}

#[tokio::test]
async fn resync_repeats_the_snapshot_and_the_pending_decision() {
    let store = MemoryStore::new();
    let handle = spawn_game_with(game_id(1), table_setup(3, 3), store, roomy()).unwrap();
    let (mut clients, holder) = start_table(&handle, 3).await;
    let (id, decision) = clients[holder].decision().await;
    let status = handle.status().await.unwrap();
    for client in &mut clients {
        client.drain();
    }

    for n in 0..3 {
        handle.send(seat(n), ClientMessage::Resync).await.unwrap();
    }
    handle.status().await.unwrap();
    for (n, client) in clients.iter_mut().enumerate() {
        let messages = client.drain();
        let Some(ServerMessage::Snapshot { seq, state }) = messages.first() else {
            panic!("seat {n} got no snapshot: {messages:?}");
        };
        assert_eq!(*seq, status.events);
        assert_eq!(state.viewer, seat(n));
        if n == holder {
            assert_eq!(
                messages[1..],
                [ServerMessage::Decision {
                    id,
                    decision: decision.clone()
                }],
                "the pending decision keeps its id"
            );
        } else {
            assert_eq!(messages.len(), 1, "only the holder gets the decision");
        }
    }
    assert_eq!(handle.status().await.unwrap(), status);
}

#[tokio::test]
async fn a_client_that_stops_reading_is_dropped_and_the_game_never_waits_for_it() {
    let store = MemoryStore::new();
    let config = GameConfig {
        seat_queue: MIN_QUEUE,
        command_queue: 64,
    };
    let handle = spawn_game_with(game_id(1), table_setup(2, 3), store, config).unwrap();
    let mut idle = Client::join(&handle, 0).await;
    let mut chatty = Client::join(&handle, 1).await;

    // The idle client never reads; the queue holds MIN_QUEUE messages and the welcome is one of them.
    let lines = MIN_QUEUE as u64 + 6;
    for n in 0..lines {
        let text = format!("line {n}");
        handle
            .send(seat(1), ClientMessage::Chat { text })
            .await
            .unwrap();
        // Keeping up with the chatter proves the actor is not stuck on the idle seat.
        loop {
            if let ServerMessage::Chat { text: got, .. } = chatty.next().await {
                assert_eq!(got, format!("line {n}"));
                break;
            }
        }
    }
    let status = handle.status().await.unwrap();
    assert_eq!(status.connected, [seat(1)], "the idle client was dropped");

    let mut buffered = 0;
    while idle.inbox.try_recv().is_ok() {
        buffered += 1;
    }
    assert_eq!(buffered, MIN_QUEUE);
    assert_eq!(
        idle.inbox.try_recv().unwrap_err(),
        TryRecvError::Disconnected
    );

    // It reconnects and is served again.
    let mut again = Client::join(&handle, 0).await;
    assert!(matches!(again.next().await, ServerMessage::Welcome { .. }));
    handle
        .send(
            seat(1),
            ClientMessage::Chat {
                text: "back".to_owned(),
            },
        )
        .await
        .unwrap();
    assert!(matches!(again.next().await, ServerMessage::Chat { .. }));
    silent(&idle.drain());
}
