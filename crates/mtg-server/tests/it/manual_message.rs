//! Goal: `ClientMessage::Manual` is taken from a seat that does not hold the pending decision as well as from one that does, shown to every seat as a batch that starts with the action and saved; the pending decision keeps its id when the change leaves it equal and is never answered for its seat; a refused action, a seat out of the game and a message before the game starts get an error to the sender alone and change nothing.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use common::{
    Client, game_id, passive_until_priority, poke, roomy, seat, silent, start_table, table_setup,
};
use mtg_engine::Event;
use mtg_engine::manual::ManualAction;
use mtg_protocol::{ClientMessage, ErrorCode, EventView, ServerMessage};
use mtg_server::{EventStore, GameHandle, MemoryStore, spawn_game_with};
use rstest::rstest;

fn new_game(players: usize) -> (GameHandle, MemoryStore) {
    let store = MemoryStore::new();
    let setup = table_setup(players, 5);
    let handle = spawn_game_with(game_id(1), setup, store.clone(), roomy()).unwrap();
    (handle, store)
}

fn life(of: usize, delta: i32) -> ClientMessage {
    ClientMessage::Manual {
        action: ManualAction::Life {
            player: seat(of),
            delta,
        },
    }
}

fn only_error(messages: &[ServerMessage]) -> ErrorCode {
    match messages {
        [ServerMessage::Error { code, .. }] => *code,
        other => panic!("expected exactly one error, got {other:?}"),
    }
}

/// The table every seat sees after `received`, checked to start with the manual action of `by`.
fn life_after(received: &[ServerMessage], by: usize, of: usize) -> i32 {
    let batch = received
        .iter()
        .find_map(|m| match m {
            ServerMessage::Events(batch) => Some(batch),
            _ => None,
        })
        .expect("a batch");
    assert!(matches!(
        batch.events.first(),
        Some(EventView::Public(Event::ManualAction { player, .. })) if *player == seat(by)
    ));
    batch.table.players[of].life
}

#[rstest]
#[case::the_seat_without_the_decision(false)]
#[case::the_seat_with_the_decision(true)]
#[tokio::test]
async fn a_manual_action_is_applied_whoever_holds_the_decision(#[case] holder_acts: bool) {
    let (handle, store) = new_game(2);
    let (mut clients, _) = start_table(&handle, 2).await;
    let (holder, id, _) = passive_until_priority(&handle, &mut clients).await;
    let actor = if holder_acts { holder } else { 1 - holder };
    let before = handle.status().await.unwrap();

    let received = poke(&handle, &mut clients, actor, life(actor, -3)).await;
    for seen in &received {
        assert_eq!(life_after(seen, actor, actor), 17);
    }
    // A life change leaves the priority decision equal: same id, not sent again, still answerable by its seat.
    for seen in &received {
        assert!(
            !seen
                .iter()
                .any(|m| matches!(m, ServerMessage::Decision { .. }))
        );
    }
    let after = handle.status().await.unwrap();
    assert_eq!(after.waiting_on, Some(seat(holder)));
    assert!(after.events > before.events);
    assert_eq!(
        store.load(game_id(1)).await.unwrap().len() as u64,
        after.events
    );
    let pass = ClientMessage::Answer {
        decision: id,
        answer: mtg_engine::Answer::Priority(mtg_engine::decision::PriorityAction::Pass),
    };
    let received = poke(&handle, &mut clients, holder, pass).await;
    assert!(
        !received[holder]
            .iter()
            .any(|m| matches!(m, ServerMessage::Error { .. }))
    );
}

#[tokio::test]
async fn a_refused_action_reaches_the_sender_alone_and_changes_nothing() {
    let (handle, store) = new_game(2);
    let (mut clients, _) = start_table(&handle, 2).await;
    let (holder, _, _) = passive_until_priority(&handle, &mut clients).await;
    let other = 1 - holder;
    let before = handle.status().await.unwrap();
    let received = poke(&handle, &mut clients, other, life(holder, -1)).await;
    assert_eq!(only_error(&received[other]), ErrorCode::IllegalAnswer);
    silent(&received[holder]);
    assert_eq!(handle.status().await.unwrap(), before);
    assert_eq!(
        store.load(game_id(1)).await.unwrap().len() as u64,
        before.events
    );
}

#[tokio::test]
async fn a_seat_out_of_the_game_cannot_act() {
    let (handle, _) = new_game(3);
    let (mut clients, _) = start_table(&handle, 3).await;
    let (holder, _, _) = passive_until_priority(&handle, &mut clients).await;
    let gone = (holder + 1) % 3;
    poke(&handle, &mut clients, gone, ClientMessage::Concede).await;
    let received = poke(&handle, &mut clients, gone, life(gone, -1)).await;
    assert_eq!(only_error(&received[gone]), ErrorCode::BadMessage);
}

#[tokio::test]
async fn a_manual_action_before_the_game_starts_is_refused() {
    let (handle, _) = new_game(2);
    let mut clients = vec![
        Client::join(&handle, 0).await,
        Client::join(&handle, 1).await,
    ];
    let received = poke(&handle, &mut clients, 0, life(0, -1)).await;
    assert!(
        received[0]
            .iter()
            .any(|m| matches!(m, ServerMessage::Error { .. }))
    );
}
