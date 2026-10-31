//! Goal: while the game runs, every other seat hears at once when a seat's client connects or goes away (a leave, a hang-up), a second socket of a seat or a spectator's socket changes nobody's presence, snapshots carry each seat's flag, and presence never touches the event log or the state hash.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use common::{Client, game_id, roomy, seat, start_table, table_setup, timed};
use mtg_core::ids::PlayerId;
use mtg_protocol::{ClientMessage, ServerMessage};
use mtg_server::{GameHandle, MemoryStore, spawn_game_with};
use rstest::rstest;

/// How the victim's client goes away.
#[derive(Debug, Clone, Copy)]
enum Gone {
    /// The connection says it leaves.
    Leave,
    /// The client hangs up without a word: its queue closes.
    HangUp,
}

async fn table(players: usize) -> (GameHandle, Vec<Client>) {
    let handle = spawn_game_with(
        game_id(1),
        table_setup(players, 3),
        MemoryStore::new(),
        roomy(),
    )
    .unwrap();
    let (mut clients, _) = start_table(&handle, players).await;
    handle.status().await.unwrap();
    for client in &mut clients {
        client.drain();
    }
    (handle, clients)
}

/// The presence messages among `messages`.
fn presence(messages: &[ServerMessage]) -> Vec<(PlayerId, bool)> {
    messages
        .iter()
        .filter_map(|m| match m {
            ServerMessage::Presence {
                seat, connected, ..
            } => Some((*seat, *connected)),
            _ => None,
        })
        .collect()
}

/// Each seat's flag in the snapshot among `messages`.
fn snapshot_flags(messages: &[ServerMessage]) -> Vec<bool> {
    messages
        .iter()
        .find_map(|m| match m {
            ServerMessage::Snapshot { state, .. } => {
                Some(state.table.players.iter().map(|p| p.connected).collect())
            }
            _ => None,
        })
        .expect("a snapshot")
}

#[rstest]
#[tokio::test]
async fn a_seat_that_goes_and_comes_back_is_told_to_every_other_seat(
    #[values(2, 3, 4)] players: usize,
    #[values(0, 1)] victim: usize,
    #[values(Gone::Leave, Gone::HangUp)] gone: Gone,
) {
    let (handle, mut clients) = table(players).await;
    let before = handle.status().await.unwrap();
    match gone {
        Gone::Leave => handle.leave(seat(victim)).await,
        Gone::HangUp => {
            let (_, inbox) = tokio::sync::mpsc::channel(1);
            drop(std::mem::replace(&mut clients[victim].inbox, inbox));
        }
    }
    for (index, client) in clients.iter_mut().enumerate() {
        if index == victim {
            continue;
        }
        let heard = timed(client.until(|m| match m {
            ServerMessage::Presence {
                seat, connected, ..
            } => Some((*seat, *connected)),
            _ => None,
        }))
        .await;
        assert_eq!(heard, (seat(victim), false));
    }
    let away = handle.status().await.unwrap();
    assert!(!away.connected.contains(&seat(victim)));

    // Another seat that asks for a snapshot reads the victim as away.
    let witness = (victim + 1) % players;
    clients[witness].drain();
    clients[witness].say(&handle, ClientMessage::Resync).await;
    handle.status().await.unwrap();
    let flags = snapshot_flags(&clients[witness].drain());
    let expected: Vec<bool> = (0..players).map(|s| s != victim).collect();
    assert_eq!(flags, expected);

    // Back again: the others hear it once, the victim reads everyone as connected and is not told of itself.
    clients[victim] = Client::join(&handle, victim).await;
    handle.status().await.unwrap();
    for (index, client) in clients.iter_mut().enumerate() {
        let messages = client.drain();
        if index == victim {
            assert_eq!(presence(&messages), []);
            assert_eq!(snapshot_flags(&messages), vec![true; players]);
        } else {
            assert_eq!(presence(&messages), [(seat(victim), true)]);
        }
    }
    let after = handle.status().await.unwrap();
    assert_eq!(after.events, before.events, "presence is not an event");
    assert_eq!(
        after.state_hash, before.state_hash,
        "presence is not hashed"
    );
}

#[rstest]
#[tokio::test]
async fn a_second_socket_of_a_seat_or_a_spectators_socket_tells_nobody(
    #[values(2, 4)] players: usize,
    #[values(0, 1)] taken: usize,
) {
    let (handle, mut clients) = table(players).await;
    // A second socket takes the seat over: the seat stays connected.
    let old = std::mem::replace(&mut clients[taken], Client::join(&handle, taken).await);
    drop(old);
    // A spectator names no seat, so it cannot join, and nobody counts it.
    assert!(handle.join(PlayerId::from_index(players)).await.is_err());
    handle.status().await.unwrap();
    for client in &mut clients {
        assert_eq!(presence(&client.drain()), []);
    }
    assert_eq!(
        handle.status().await.unwrap().connected,
        (0..players).map(seat).collect::<Vec<_>>()
    );
}

#[rstest]
#[tokio::test]
async fn a_batch_after_a_drop_carries_the_flag(#[values(2, 3)] players: usize) {
    let (handle, mut clients) = table(players).await;
    let victim = players - 1;
    handle.leave(seat(victim)).await;
    handle.status().await.unwrap();
    // Seat 0 concedes: every other connected seat gets a batch whose table reads the victim as away.
    clients[0].say(&handle, ClientMessage::Concede).await;
    handle.status().await.unwrap();
    let witness = usize::from(players > 2);
    let tables: Vec<Vec<bool>> = clients[witness]
        .drain()
        .iter()
        .filter_map(|m| match m {
            ServerMessage::Events(batch) => {
                Some(batch.table.players.iter().map(|p| p.connected).collect())
            }
            _ => None,
        })
        .collect();
    assert!(!tables.is_empty(), "a batch reached seat {witness}");
    let expected: Vec<bool> = (0..players).map(|s| s != victim).collect();
    assert!(tables.iter().all(|flags| *flags == expected), "{tables:?}");
}

const THINK: std::time::Duration = std::time::Duration::from_secs(60);

/// The opening decisions run on a one-minute think clock.
fn think_clock() -> mtg_server::GameConfig {
    let pace = mtg_server::Pace {
        think: Some(THINK),
        ..roomy().pace
    };
    mtg_server::GameConfig { pace, ..roomy() }
}

/// The presence messages among `messages` with the time each gives.
fn away_times(messages: &[ServerMessage]) -> Vec<(PlayerId, bool, Option<u64>)> {
    messages
        .iter()
        .filter_map(|m| match m {
            ServerMessage::Presence {
                seat,
                connected,
                deadline_ms,
            } => Some((*seat, *connected, *deadline_ms)),
            _ => None,
        })
        .collect()
}

#[rstest]
#[tokio::test]
async fn a_seat_away_on_a_clock_comes_with_the_time_before_the_game_answers_for_it(
    #[values(2, 3)] players: usize,
    #[values(0, 1)] victim: usize,
) {
    let setup = table_setup(players, 3);
    let handle = spawn_game_with(game_id(1), setup, MemoryStore::new(), think_clock()).unwrap();
    // Every seat is asked about its opening hand on the think clock.
    let (mut clients, _) = start_table(&handle, players).await;
    handle.status().await.unwrap();
    let witness = (victim + 1) % players;
    clients[witness].drain();
    handle.leave(seat(victim)).await;
    handle.status().await.unwrap();
    let heard = away_times(&clients[witness].drain());
    let [(who, false, Some(ms))] = heard[..] else {
        panic!("one away notice with a time, got {heard:?}");
    };
    assert_eq!(who, seat(victim));
    assert!(ms > 0 && ms <= 60_000, "{ms}");

    // A client that joins hears the time of the seat that is away, and its snapshot says who that is.
    clients[witness] = Client::join(&handle, witness).await;
    handle.status().await.unwrap();
    let messages = clients[witness].drain();
    let heard = away_times(&messages);
    let [(who, false, Some(again))] = heard[..] else {
        panic!("one away notice with a time, got {heard:?}");
    };
    assert_eq!(who, seat(victim));
    assert!(again <= ms, "{again} > {ms}");
    let expected: Vec<bool> = (0..players).map(|s| s != victim).collect();
    assert_eq!(snapshot_flags(&messages), expected);
}
