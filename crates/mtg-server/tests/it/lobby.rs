//! Goal: before the start the actor only collects joins, readiness and chat; the engine appears exactly when the last seat is ready, and the opening events reach every seat from seq 0.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use common::{Client, game_id, poke, roomy, roster, seat, silent, table_setup};
use mtg_engine::{GameSetup, replay};
use mtg_protocol::{ClientMessage, ErrorCode, FullControlMode, PROTOCOL_VERSION, ServerMessage};
use mtg_server::{
    EventStore, GameHandle, JoinError, MAX_CHAT_CHARS, MemoryStore, StartError, spawn_game,
    spawn_game_with,
};
use mtg_testkit::fuzz_setup;

fn lobby(players: usize) -> (GameHandle, MemoryStore) {
    let store = MemoryStore::new();
    let handle =
        spawn_game_with(game_id(7), table_setup(players, 3), store.clone(), roomy()).unwrap();
    (handle, store)
}

#[tokio::test]
async fn welcome_names_the_game_the_seat_and_the_table_size() {
    let (handle, _) = lobby(3);
    let mut client = Client::join(&handle, 1).await;
    assert_eq!(
        client.next().await,
        ServerMessage::Welcome {
            protocol_version: PROTOCOL_VERSION,
            game: game_id(7).to_string(),
            player: seat(1),
            seats: 3,
            // No auto-pass is the Held mode.
            auto_pass: false,
            yielding: None,
            stops: Vec::new(),
            full_control: FullControlMode::Held,
        }
    );
    // The roster follows the welcome: only this seat is here and nobody is ready.
    assert_eq!(
        client.next().await,
        roster(&[(false, false), (true, false), (false, false)])
    );
    silent(&client.drain());
    let status = handle.status().await.unwrap();
    assert!(!status.started);
    assert_eq!(status.connected, [seat(1)]);
    assert_eq!(status.waiting_on, None);
}

#[tokio::test]
async fn a_seat_that_does_not_exist_cannot_be_joined() {
    let (handle, _) = lobby(2);
    assert_eq!(
        handle.join(seat(2)).await.unwrap_err(),
        JoinError::NoSuchSeat(seat(2))
    );
    assert_eq!(
        handle.join(seat(99)).await.unwrap_err(),
        JoinError::NoSuchSeat(seat(99))
    );
    assert_eq!(handle.status().await.unwrap().connected, []);
}

#[tokio::test]
async fn the_engine_exists_only_once_every_seat_is_ready() {
    let (handle, store) = lobby(3);
    let mut clients = Vec::new();
    for n in 0..3 {
        clients.push(Client::join(&handle, n).await);
    }
    for client in &mut clients {
        client.drain();
    }

    // Two ready seats do not start anything. The first press of each changes the roster, so everyone hears of it ...
    for n in 0..2 {
        let received = poke(&handle, &mut clients, n, ClientMessage::Ready).await;
        let flags: Vec<(bool, bool)> = (0..3).map(|seat| (true, seat <= n)).collect();
        for messages in &received {
            assert_eq!(messages.as_slice(), [roster(&flags)]);
        }
    }
    // ... and pressing again, as often as they like, changes nothing, so nobody hears anything.
    for _ in 0..3 {
        for n in 0..2 {
            let received = poke(&handle, &mut clients, n, ClientMessage::Ready).await;
            for messages in &received {
                silent(messages);
            }
        }
    }
    let status = handle.status().await.unwrap();
    assert!(!status.started);
    assert_eq!(store.load(game_id(7)).await.unwrap(), []);

    let received = poke(&handle, &mut clients, 2, ClientMessage::Ready).await;
    let status = handle.status().await.unwrap();
    assert!(status.started);
    assert!(status.events > 0);
    let opening = store.load(game_id(7)).await.unwrap();
    assert_eq!(opening.len() as u64, status.events);
    assert_eq!(
        status.state_hash,
        Some(replay(&table_setup(3, 3), &opening).unwrap().state_hash())
    );

    let mut deciding = 0;
    for messages in &received {
        let Some(ServerMessage::Events(batch)) = messages.first() else {
            panic!("the opening events come first: {messages:?}");
        };
        assert_eq!(batch.from_seq, 0);
        assert_eq!(batch.events.len(), opening.len());
        deciding += messages
            .iter()
            .filter(|m| matches!(m, ServerMessage::Decision { .. }))
            .count();
    }
    assert_eq!(deciding, 1, "exactly one seat is asked to decide");
    let waiting = status.waiting_on.unwrap().index();
    assert!(matches!(
        received[waiting].last(),
        Some(ServerMessage::Decision { .. })
    ));

    // A ready that arrives after the start is harmless.
    let received = poke(&handle, &mut clients, 0, ClientMessage::Ready).await;
    for messages in &received {
        silent(messages);
    }
}

#[tokio::test]
async fn chat_reaches_everyone_connected_and_is_never_stored() {
    let (handle, store) = lobby(2);
    let mut clients = vec![
        Client::join(&handle, 0).await,
        Client::join(&handle, 1).await,
    ];
    let say = |text: &str| ClientMessage::Chat {
        text: text.to_owned(),
    };

    let received = poke(&handle, &mut clients, 1, say("hello table")).await;
    for messages in &received {
        assert_eq!(
            messages.as_slice(),
            [ServerMessage::Chat {
                from: seat(1),
                text: "hello table".to_owned()
            }],
            "the sender sees its own line too, in order"
        );
    }

    // The same once the game is running.
    for client in &clients {
        client.say(&handle, ClientMessage::Ready).await;
    }
    handle.status().await.unwrap();
    let saved = store.load(game_id(7)).await.unwrap().len();
    let received = poke(&handle, &mut clients, 0, say("good luck")).await;
    for messages in &received {
        assert!(
            matches!(messages.as_slice(), [ServerMessage::Chat { text, .. }] if text == "good luck")
        );
    }
    assert_eq!(store.load(game_id(7)).await.unwrap().len(), saved);

    // Absurdly long lines are refused instead of being relayed.
    let long = "x".repeat(MAX_CHAT_CHARS + 1);
    let received = poke(&handle, &mut clients, 0, say(&long)).await;
    assert!(matches!(
        received[0].as_slice(),
        [ServerMessage::Error {
            code: ErrorCode::BadMessage,
            ..
        }]
    ));
    silent(&received[1]);
    let fits = "y".repeat(MAX_CHAT_CHARS);
    let received = poke(&handle, &mut clients, 0, say(&fits)).await;
    assert!(matches!(
        received[1].as_slice(),
        [ServerMessage::Chat { .. }]
    ));
}

#[tokio::test]
async fn a_setup_that_cannot_be_played_is_refused_at_the_door() {
    let store = MemoryStore::new();
    let mut one_seat: GameSetup = fuzz_setup(2, 1);
    one_seat.players.truncate(1);
    assert!(matches!(
        spawn_game(game_id(1), one_seat, store.clone()),
        Err(StartError::Setup(_))
    ));
    let mut short_deck: GameSetup = fuzz_setup(2, 1);
    short_deck.players[1].library.truncate(10);
    assert!(matches!(
        spawn_game(game_id(2), short_deck, store),
        Err(StartError::Setup(_))
    ));
}

#[tokio::test]
async fn a_game_id_that_already_has_a_log_cannot_be_started_over_it() {
    let (handle, store) = lobby(2);
    let first = mtg_engine::Event::FirstPlayerChosen { player: seat(0) };
    store.append(game_id(7), 0, &[first]).await.unwrap();
    let mut clients = vec![
        Client::join(&handle, 0).await,
        Client::join(&handle, 1).await,
    ];
    for client in &mut clients {
        client.drain();
    }
    poke(&handle, &mut clients, 0, ClientMessage::Ready).await;
    let received = poke(&handle, &mut clients, 1, ClientMessage::Ready).await;

    // The last seat is told the start failed; the lobby stays as it was, with everybody ready, and everyone sees that.
    let everybody = roster(&[(true, true), (true, true)]);
    assert!(matches!(
        received[1].as_slice(),
        [
            ServerMessage::Error {
                code: ErrorCode::Internal,
                ..
            },
            roster
        ] if *roster == everybody
    ));
    assert_eq!(received[0].as_slice(), [everybody]);
    assert!(!handle.status().await.unwrap().started);
    assert_eq!(
        store.load(game_id(7)).await.unwrap().len(),
        1,
        "the old log is untouched"
    );
}
