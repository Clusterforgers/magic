//! Goal: when the store refuses a write, the move is answered with an internal error to its sender alone, the game is exactly as before (decision, id and half-finished cast included), nothing is broadcast, and the same move succeeds once the store recovers.
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use common::{
    Client, FlakyStore, game_id, passive, passive_until_priority, poke, roomy, seat, silent,
    start_table, table_setup,
};
use mtg_engine::decision::{DecisionKind, PayChoice, PriorityAction};
use mtg_engine::{Answer, Event, GameSetup, replay};
use mtg_protocol::{ClientMessage, ErrorCode, ServerMessage};
use mtg_server::{EventStore, GameHandle, spawn_game_with};
use mtg_testkit::cards::{bear, forest};
use mtg_testkit::{Scenario, ready};

fn new_game(setup: GameSetup) -> (GameHandle, FlakyStore) {
    let store = FlakyStore::new();
    let handle = spawn_game_with(game_id(1), setup, store.clone(), roomy()).unwrap();
    (handle, store)
}

/// The one internal error in `messages`, whose text must not leak what failed.
fn internal_error(messages: &[ServerMessage]) {
    match messages {
        [ServerMessage::Error { code, message }] => {
            assert_eq!(*code, ErrorCode::Internal);
            assert!(
                !message.contains("injected"),
                "store details leaked: {message}"
            );
        }
        other => panic!("expected one internal error, got {other:?}"),
    }
}

#[tokio::test]
async fn a_failed_save_leaves_the_game_unchanged_and_the_same_move_works_after_recovery() {
    let setup = table_setup(3, 9);
    let (handle, store) = new_game(setup.clone());
    let (mut clients, _) = start_table(&handle, 3).await;
    let (holder, id, decision) = passive_until_priority(&handle, &mut clients).await;
    let before = handle.status().await.unwrap();
    let saved = store.inner().load(game_id(1)).await.unwrap();
    let pass = ClientMessage::Answer {
        decision: id,
        answer: passive(&decision),
    };

    store.fail(true);
    for _ in 0..3 {
        let received = poke(&handle, &mut clients, holder, pass.clone()).await;
        internal_error(&received[holder]);
        for (n, messages) in received.iter().enumerate() {
            if n != holder {
                silent(messages);
            }
        }
        assert_eq!(handle.status().await.unwrap(), before);
        assert_eq!(store.inner().load(game_id(1)).await.unwrap(), saved);
    }

    store.fail(false);
    let received = poke(&handle, &mut clients, holder, pass).await;
    assert!(
        received
            .iter()
            .all(|m| matches!(m.first(), Some(ServerMessage::Events(_))))
    );
    let after = handle.status().await.unwrap();
    assert!(after.events > before.events);
    let logged = store.inner().load(game_id(1)).await.unwrap();
    assert_eq!(logged.len() as u64, after.events);
    assert_eq!(
        Some(replay(&setup, &logged).unwrap().state_hash()),
        after.state_hash
    );
}

#[tokio::test]
async fn a_failed_save_at_the_start_keeps_the_lobby_and_ready_tries_again() {
    let (handle, store) = new_game(table_setup(2, 4));
    store.fail(true);
    let mut clients = vec![
        Client::join(&handle, 0).await,
        Client::join(&handle, 1).await,
    ];
    for client in &mut clients {
        client.drain();
    }
    handle.send(seat(0), ClientMessage::Ready).await.unwrap();
    handle.send(seat(1), ClientMessage::Ready).await.unwrap();
    handle.status().await.unwrap();

    internal_error(&clients[1].drain());
    silent(&clients[0].drain());
    let status = handle.status().await.unwrap();
    assert!(!status.started);
    assert_eq!(status.events, 0);
    assert_eq!(store.inner().load(game_id(1)).await.unwrap(), []);

    store.fail(false);
    handle.send(seat(1), ClientMessage::Ready).await.unwrap();
    let status = handle.status().await.unwrap();
    assert!(status.started);
    let logged = store.inner().load(game_id(1)).await.unwrap();
    assert_eq!(logged.len() as u64, status.events);
    for client in &mut clients {
        let messages = client.drain();
        assert!(
            matches!(messages.first(), Some(ServerMessage::Events(batch)) if batch.from_seq == 0),
            "the opening events arrive from seq 0: {messages:?}"
        );
    }
}

#[tokio::test]
async fn a_failed_save_of_a_concession_keeps_the_player_in_the_game() {
    let (handle, store) = new_game(table_setup(3, 5));
    let (mut clients, _) = start_table(&handle, 3).await;
    let before = handle.status().await.unwrap();

    store.fail(true);
    let received = poke(&handle, &mut clients, 2, ClientMessage::Concede).await;
    internal_error(&received[2]);
    silent(&received[0]);
    silent(&received[1]);
    assert_eq!(handle.status().await.unwrap(), before);

    store.fail(false);
    let received = poke(&handle, &mut clients, 2, ClientMessage::Concede).await;
    for messages in &received {
        let Some(ServerMessage::Events(batch)) = messages.first() else {
            panic!("expected events first: {messages:?}");
        };
        assert!(batch.events.iter().any(|e| matches!(
            e,
            mtg_protocol::EventView::Public(Event::PlayerLost { player, .. }) if *player == seat(2)
        )));
    }
}

#[tokio::test]
async fn a_failed_save_keeps_a_half_finished_cast_payable() {
    let setup = Scenario::new(2)
        .hand(0, [bear()])
        .battlefield(0, [ready(forest()), ready(forest())])
        .setup()
        .unwrap();
    let (handle, store) = new_game(setup);
    let (mut clients, holder) = start_table(&handle, 2).await;
    assert_eq!(holder, 0);

    let (id, decision) = clients[0].decision().await;
    let DecisionKind::Priority(options) = &decision.kind else {
        panic!("expected priority, got {decision:?}");
    };
    let cast = Answer::Priority(PriorityAction::CastSpell(options.castable[0]));
    handle
        .send(
            seat(0),
            ClientMessage::Answer {
                decision: id,
                answer: cast,
            },
        )
        .await
        .unwrap();
    let (pay_id, pay) = clients[0].decision().await;
    assert!(matches!(pay.kind, DecisionKind::PayMana(_)));
    assert!(pay_id > id);
    let before = handle.status().await.unwrap();

    store.fail(true);
    let payment = ClientMessage::Answer {
        decision: pay_id,
        answer: Answer::PayMana(PayChoice::Auto),
    };
    let received = poke(&handle, &mut clients, 0, payment.clone()).await;
    internal_error(&received[0]);
    silent(&received[1]);
    assert_eq!(handle.status().await.unwrap(), before);

    store.fail(false);
    let received = poke(&handle, &mut clients, 0, payment).await;
    let Some(ServerMessage::Events(batch)) = received[1].first() else {
        panic!("the opponent should see the cast: {:?}", received[1]);
    };
    assert!(
        batch
            .events
            .iter()
            .any(|e| matches!(e, mtg_protocol::EventView::Public(Event::SpellCast { .. })))
    );
}
