//! Goal: before the start every connected seat is kept told who is at the table (name, connected, ready, and how many seats still have to press Ready) on connect and after each join, hang-up, Ready and `NotReady`, and only after a change; `NotReady` is harmless once the game runs, the last Ready starts the game without a roster, and a resumed lobby starts with nobody ready.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use common::{
    Client, game_id, passive, poke, roomy, roster, seat, silent, start_table, table_setup, timed,
    without_rosters,
};
use mtg_core::ids::PlayerId;
use mtg_protocol::{ClientMessage, ServerMessage};
use mtg_server::{
    EventStore, GameConfig, GameHandle, MIN_QUEUE, MemoryStore, Pace, Registry, ServerConfig,
    resume_game_with, spawn_game_with,
};
use rstest::rstest;

fn lobby(players: usize) -> (GameHandle, MemoryStore) {
    let store = MemoryStore::new();
    let handle =
        spawn_game_with(game_id(7), table_setup(players, 3), store.clone(), roomy()).unwrap();
    (handle, store)
}

/// Joins every seat and throws away what the joins sent.
async fn seated(handle: &GameHandle, seats: usize) -> Vec<Client> {
    let mut clients = Vec::new();
    for n in 0..seats {
        clients.push(Client::join(handle, n).await);
    }
    for client in &mut clients {
        client.drain();
    }
    clients
}

/// The roster in `messages`, which must hold exactly that one.
fn only_roster(messages: &[ServerMessage]) -> &ServerMessage {
    match messages {
        [roster @ ServerMessage::Lobby { .. }] => roster,
        other => panic!("expected one roster, got {other:?}"),
    }
}

#[tokio::test]
async fn a_join_is_answered_with_the_welcome_and_the_roster_and_told_to_the_others() {
    let (handle, _) = lobby(3);

    let mut second = Client::join(&handle, 1).await;
    assert!(matches!(second.next().await, ServerMessage::Welcome { .. }));
    assert_eq!(
        second.drain(),
        [roster(&[(false, false), (true, false), (false, false)])]
    );

    let mut first = Client::join(&handle, 0).await;
    let both = roster(&[(true, false), (true, false), (false, false)]);
    assert!(matches!(first.next().await, ServerMessage::Welcome { .. }));
    assert_eq!(
        first.drain(),
        std::slice::from_ref(&both),
        "the joiner is told too"
    );
    assert_eq!(second.drain(), [both], "and so is the seat that was here");

    let mut third = Client::join(&handle, 2).await;
    let everyone = roster(&[(true, false); 3]);
    assert!(matches!(third.next().await, ServerMessage::Welcome { .. }));
    for client in [&mut first, &mut second, &mut third] {
        assert_eq!(client.drain(), std::slice::from_ref(&everyone));
    }
}

#[rstest]
#[case::two_seats(2)]
#[case::three_seats(3)]
#[case::six_seats(6)]
#[tokio::test]
async fn ready_and_not_ready_are_pushed_to_every_seat_and_only_when_they_change(
    #[case] seats: usize,
) {
    let (handle, _) = lobby(seats);
    let mut clients = seated(&handle, seats).await;
    let mut flags = vec![(true, false); seats];

    // (seat, message, does it change the roster?) - a repeat or a take-back of nothing is not news. At most one of
    // the first two seats is ready at a time, so not even a table of two starts.
    let script = [
        (0, ClientMessage::Ready, true),
        (0, ClientMessage::Ready, false),
        (1, ClientMessage::NotReady, false),
        (0, ClientMessage::NotReady, true),
        (0, ClientMessage::NotReady, false),
        (1, ClientMessage::Ready, true),
        (1, ClientMessage::Ready, false),
        (1, ClientMessage::NotReady, true),
        (0, ClientMessage::Ready, true),
    ];
    for (who, message, changes) in script {
        let received = poke(&handle, &mut clients, who, message.clone()).await;
        if changes {
            flags[who].1 = matches!(message, ClientMessage::Ready);
            for messages in &received {
                assert_eq!(only_roster(messages), &roster(&flags));
            }
        } else {
            for messages in &received {
                silent(messages);
            }
        }
    }
    assert!(!handle.status().await.unwrap().started, "not everyone is");
}

#[tokio::test]
async fn needed_counts_the_seats_that_still_have_to_press_ready() {
    let (handle, _) = lobby(4);
    let mut clients = seated(&handle, 4).await;
    let mut needed = Vec::new();
    for (who, message) in [
        (0, ClientMessage::Ready),
        (1, ClientMessage::Ready),
        (1, ClientMessage::NotReady),
        (2, ClientMessage::Ready),
        (0, ClientMessage::NotReady),
    ] {
        let received = poke(&handle, &mut clients, who, message).await;
        let ServerMessage::Lobby { seats, needed: n } = only_roster(&received[3]).clone() else {
            unreachable!()
        };
        assert_eq!(
            n as usize,
            seats.iter().filter(|s| !s.ready).count(),
            "needed is the seats that are not ready"
        );
        needed.push(n);
    }
    assert_eq!(needed, [3, 2, 3, 2, 3]);
}

#[tokio::test]
async fn a_client_that_hangs_up_is_noticed_without_a_word_and_the_others_are_told() {
    let (handle, _) = lobby(3);
    let mut clients = seated(&handle, 3).await;
    clients[1].say(&handle, ClientMessage::Ready).await;
    handle.status().await.unwrap();
    for client in &mut clients {
        client.drain();
    }

    // The socket of seat 1 ends: its queue is dropped, and no message is sent to the game.
    drop(clients.remove(1));
    let told = timed(clients[0].inbox.recv()).await.unwrap();
    assert_eq!(told, roster(&[(true, false), (false, true), (true, false)]));
    let told = timed(clients[1].inbox.recv()).await.unwrap();
    assert_eq!(told, roster(&[(true, false), (false, true), (true, false)]));
    silent(&clients[0].drain());
    assert_eq!(handle.status().await.unwrap().connected, [seat(0), seat(2)]);

    // It comes back and everyone hears that too; its Ready was never taken back.
    let mut back = Client::join(&handle, 1).await;
    let everyone = roster(&[(true, false), (true, true), (true, false)]);
    assert!(matches!(back.next().await, ServerMessage::Welcome { .. }));
    assert_eq!(back.drain(), std::slice::from_ref(&everyone));
    assert_eq!(clients[0].drain(), [everyone]);
}

#[tokio::test]
async fn leaving_is_told_like_a_hang_up_and_leaving_twice_is_not_news() {
    let (handle, _) = lobby(2);
    let mut clients = seated(&handle, 2).await;

    handle.leave(seat(1)).await;
    handle.status().await.unwrap();
    assert_eq!(
        clients[0].drain(),
        [roster(&[(true, false), (false, false)])]
    );
    handle.leave(seat(1)).await;
    handle.leave(seat(1)).await;
    handle.status().await.unwrap();
    silent(&clients[0].drain());
    assert_eq!(handle.status().await.unwrap().connected, [seat(0)]);
}

#[tokio::test]
async fn a_seat_that_was_ready_and_left_stays_ready_and_does_not_hold_the_start_up() {
    let (handle, _) = lobby(3);
    let mut clients = seated(&handle, 3).await;
    clients[2].say(&handle, ClientMessage::Ready).await;
    handle.status().await.unwrap();
    drop(clients.remove(2));
    handle.status().await.unwrap();
    let status = handle.status().await.unwrap();
    assert_eq!(status.ready, [seat(2)]);
    assert_eq!(status.connected, [seat(0), seat(1)]);

    // The roster says so: away, but ready, so two seats are still needed.
    let latest = clients[0].drain().pop().unwrap();
    assert_eq!(
        latest,
        roster(&[(true, false), (true, false), (false, true)])
    );
    let ServerMessage::Lobby { needed, .. } = latest else {
        unreachable!()
    };
    assert_eq!(needed, 2);

    // The two who are here start the game; the one who is away finds it running when it returns.
    for client in &clients {
        client.say(&handle, ClientMessage::Ready).await;
    }
    assert!(handle.status().await.unwrap().started);
}

#[tokio::test]
async fn a_connection_that_takes_a_seat_over_is_told_alone() {
    let (handle, _) = lobby(2);
    let mut clients = seated(&handle, 2).await;

    let mut newer = Client::join(&handle, 1).await;
    assert!(matches!(newer.next().await, ServerMessage::Welcome { .. }));
    assert_eq!(
        newer.drain(),
        [roster(&[(true, false), (true, false)])],
        "the new connection learns who is here"
    );
    silent(&clients[0].drain());
    assert!(
        clients[1].inbox.try_recv().is_err(),
        "the old one was closed with nothing new"
    );
    assert_eq!(handle.status().await.unwrap().connected, [seat(0), seat(1)]);
}

#[tokio::test]
async fn taking_ready_back_stops_the_last_ready_from_starting_the_game() {
    let (handle, store) = lobby(3);
    let mut clients = seated(&handle, 3).await;
    for n in [0, 1] {
        poke(&handle, &mut clients, n, ClientMessage::Ready).await;
    }
    poke(&handle, &mut clients, 1, ClientMessage::NotReady).await;
    poke(&handle, &mut clients, 2, ClientMessage::Ready).await;
    assert!(!handle.status().await.unwrap().started);
    assert_eq!(store.load(game_id(7)).await.unwrap(), []);

    let received = poke(&handle, &mut clients, 1, ClientMessage::Ready).await;
    assert!(handle.status().await.unwrap().started);
    for messages in &received {
        assert!(
            matches!(messages.first(), Some(ServerMessage::Events(batch)) if batch.from_seq == 0),
            "the opening events come first: {messages:?}"
        );
        assert!(
            without_rosters(messages.clone()).len() == messages.len(),
            "the start is announced by the game, not by a roster: {messages:?}"
        );
    }
}

#[tokio::test]
async fn ready_and_not_ready_after_the_start_are_harmless() {
    let (handle, _) = lobby(3);
    let (mut clients, waiting) = start_table(&handle, 3).await;
    let (id, decision) = clients[waiting].decision().await;
    for client in &mut clients {
        client.drain();
    }
    let before = handle.status().await.unwrap();
    assert!(before.started);
    assert_eq!(before.ready, [seat(0), seat(1), seat(2)]);

    for message in [ClientMessage::Ready, ClientMessage::NotReady] {
        for who in 0..3 {
            let received = poke(&handle, &mut clients, who, message.clone()).await;
            for messages in &received {
                silent(messages);
            }
        }
    }
    assert_eq!(
        handle.status().await.unwrap(),
        before,
        "the game is as it was"
    );

    // And the game still plays: the seat that is asked can answer.
    assert_eq!(before.waiting_on, Some(seat(waiting)));
    clients[waiting]
        .say(
            &handle,
            ClientMessage::Answer {
                decision: id,
                answer: passive(&decision),
            },
        )
        .await;
    assert!(handle.status().await.unwrap().events > before.events);
}

#[tokio::test]
async fn a_message_from_a_seat_that_does_not_exist_is_ignored() {
    let (handle, _) = lobby(2);
    let mut clients = seated(&handle, 2).await;
    for message in [ClientMessage::Ready, ClientMessage::NotReady] {
        handle.send(seat(9), message).await.unwrap();
    }
    handle.status().await.unwrap();
    for client in &mut clients {
        silent(&client.drain());
    }
    assert_eq!(handle.status().await.unwrap().ready, Vec::<PlayerId>::new());
}

#[tokio::test]
async fn the_status_lists_the_ready_seats_in_seat_order() {
    let (handle, _) = lobby(4);
    let mut clients = seated(&handle, 4).await;
    assert_eq!(handle.status().await.unwrap().ready, []);
    for n in [3, 0, 2] {
        poke(&handle, &mut clients, n, ClientMessage::Ready).await;
    }
    assert_eq!(
        handle.status().await.unwrap().ready,
        [seat(0), seat(2), seat(3)]
    );
    poke(&handle, &mut clients, 2, ClientMessage::NotReady).await;
    assert_eq!(handle.status().await.unwrap().ready, [seat(0), seat(3)]);
}

#[tokio::test]
async fn a_client_that_cannot_keep_up_is_dropped_and_the_others_are_told_of_the_gap() {
    let config = GameConfig {
        seat_queue: MIN_QUEUE,
        command_queue: 64,
        pace: Pace::MANUAL,
    };
    let handle =
        spawn_game_with(game_id(7), table_setup(3, 3), MemoryStore::new(), config).unwrap();
    let mut idle = Client::join(&handle, 0).await; // never reads: welcome and roster
    let mut keen = Client::join(&handle, 1).await; // one more roster for the idle seat
    handle.status().await.unwrap();

    // Two changes more: the idle queue takes the first and overflows on the second. The keen seat reads all it is sent.
    keen.drain();
    keen.say(&handle, ClientMessage::Ready).await;
    handle.status().await.unwrap();
    keen.drain();
    keen.say(&handle, ClientMessage::NotReady).await;
    handle.status().await.unwrap();

    assert_eq!(handle.status().await.unwrap().connected, [seat(1)]);
    let last = keen.drain().pop().unwrap();
    assert_eq!(
        last,
        roster(&[(false, false), (true, false), (false, false)]),
        "the seat that was dropped is shown as gone"
    );
    let mut queued = 0;
    while idle.inbox.try_recv().is_ok() {
        queued += 1;
    }
    assert_eq!(queued, MIN_QUEUE, "it still has what fitted");
}

#[tokio::test]
async fn names_are_the_ones_the_game_was_made_with_and_missing_ones_are_called_seat_n() {
    let config = ServerConfig {
        game: roomy(),
        ..ServerConfig::default()
    };
    let registry = Registry::new(&config);
    let created = registry
        .register(table_setup(3, 3), vec!["Ada".into(), "Bo".into()])
        .await
        .unwrap();
    let handle = registry.handle(created.id).unwrap();
    let mut client = Client::join(&handle, 2).await;
    assert!(matches!(client.next().await, ServerMessage::Welcome { .. }));
    let ServerMessage::Lobby { seats, .. } = client.next().await else {
        panic!("the roster follows the welcome");
    };
    let names: Vec<&str> = seats.iter().map(|s| s.name.as_str()).collect();
    assert_eq!(names, ["Ada", "Bo", "Seat 3"]);
    let seats_in_order: Vec<usize> = seats.iter().map(|s| s.seat.index()).collect();
    assert_eq!(seats_in_order, [0, 1, 2]);
}

#[tokio::test]
async fn a_lobby_resumed_from_an_empty_log_starts_with_nobody_ready() {
    let (handle, store) = lobby(2);
    let mut clients = seated(&handle, 2).await;
    poke(&handle, &mut clients, 0, ClientMessage::Ready).await;
    assert_eq!(handle.status().await.unwrap().ready, [seat(0)]);
    drop((handle, clients));

    // The process restarted: readiness was never stored, so the resumed lobby asks again.
    let resumed = resume_game_with(store, game_id(7), table_setup(2, 3), roomy())
        .await
        .unwrap();
    let status = resumed.status().await.unwrap();
    assert!(!status.started);
    assert_eq!(status.ready, []);
    let mut client = Client::join(&resumed, 0).await;
    assert!(matches!(client.next().await, ServerMessage::Welcome { .. }));
    assert_eq!(client.drain(), [roster(&[(true, false), (false, false)])]);
}

#[tokio::test]
async fn a_game_resumed_from_its_log_runs_and_sends_no_roster() {
    let (handle, store) = lobby(2);
    let (clients, _) = start_table(&handle, 2).await;
    drop((handle, clients));

    let resumed = resume_game_with(store, game_id(7), table_setup(2, 3), roomy())
        .await
        .unwrap();
    let status = resumed.status().await.unwrap();
    assert!(status.started);
    assert_eq!(status.ready, [seat(0), seat(1)]);
    let mut client = Client::join(&resumed, 0).await;
    assert!(matches!(client.next().await, ServerMessage::Welcome { .. }));
    let messages = client.drain();
    assert!(
        matches!(messages.first(), Some(ServerMessage::Snapshot { .. })),
        "a snapshot follows the welcome: {messages:?}"
    );
    assert_eq!(without_rosters(messages.clone()), messages);
    // A late NotReady changes nothing in it either.
    client.say(&resumed, ClientMessage::NotReady).await;
    resumed.status().await.unwrap();
    silent(&client.drain());
}

#[tokio::test]
async fn a_command_that_meets_the_closed_queue_first_does_not_hide_the_hang_up() {
    let (handle, _) = lobby(3);
    let mut clients = seated(&handle, 3).await;

    // On this single-threaded runtime the actor does not run before the status request is queued, so the request
    // is the first thing to find the closed queue (it sweeps it, as a chat line or a lookup from the web would).
    drop(clients.remove(2));
    assert_eq!(handle.status().await.unwrap().connected, [seat(0), seat(1)]);
    let gone = roster(&[(true, false), (true, false), (false, false)]);
    for client in &mut clients {
        assert_eq!(timed(client.inbox.recv()).await.unwrap(), gone);
        silent(&client.drain());
    }
}

#[tokio::test]
async fn chat_that_meets_the_closed_queue_first_does_not_hide_the_hang_up_either() {
    let (handle, _) = lobby(2);
    let mut clients = seated(&handle, 2).await;
    drop(clients.remove(1));
    let line = ClientMessage::Chat {
        text: "anyone there?".to_owned(),
    };
    clients[0].say(&handle, line).await;
    handle.status().await.unwrap();
    let told = clients[0].drain();
    assert_eq!(told.len(), 2, "the line and the roster: {told:?}");
    assert!(told.contains(&ServerMessage::Chat {
        from: seat(0),
        text: "anyone there?".to_owned()
    }));
    assert!(told.contains(&roster(&[(true, false), (false, false)])));
}
