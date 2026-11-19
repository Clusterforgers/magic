//! Goal: each seat is told the pictures of its own deck as the game starts and again when it reconnects, a spectator only the public ones, and no message to a seat names a card of another seat's hidden deck.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use crate::common;

use common::{Client, game_id, roomy, table_setup};
use mtg_cards::{CardDefinition, CardIdentity};
use mtg_engine::GameSetup;
use mtg_protocol::{ClientMessage, ServerMessage};
use mtg_server::{GameHandle, MemoryStore, spawn_game_with};

/// The fuzz table with every library card pictured as `s<seat>-<index>`.
fn pictured(players: usize) -> GameSetup {
    let mut setup = table_setup(players, 3);
    for (at, seat) in setup.players.iter_mut().enumerate() {
        for (index, card) in seat.library.iter_mut().enumerate() {
            let mut def: CardDefinition = (**card).clone();
            def.identity = Some(CardIdentity {
                oracle_id: format!("s{at}-{index}"),
                printing_id: None,
            });
            *card = Arc::new(def);
        }
    }
    setup
}

fn started(players: usize) -> GameHandle {
    spawn_game_with(game_id(31), pictured(players), MemoryStore::new(), roomy()).unwrap()
}

/// The oracle ids of the one `DeckPictures` among `messages`.
fn told(messages: &[ServerMessage]) -> Vec<String> {
    let lists: Vec<_> = messages
        .iter()
        .filter_map(|message| match message {
            ServerMessage::DeckPictures { cards } => Some(cards),
            _ => None,
        })
        .collect();
    assert_eq!(lists.len(), 1, "one list in {messages:?}");
    lists[0]
        .iter()
        .map(|card| card.identity.oracle_id.clone())
        .collect()
}

/// Whether any message to this client names a card of seat `other`'s library.
fn names_seat(messages: &[ServerMessage], other: usize) -> bool {
    let text = serde_json::to_string(messages).unwrap();
    text.contains(&format!("\"s{other}-"))
}

#[tokio::test]
async fn each_seat_is_told_its_own_deck_at_the_start_and_on_reconnect() {
    let handle = started(2);
    let mut clients = vec![
        Client::join(&handle, 0).await,
        Client::join(&handle, 1).await,
    ];
    for client in &mut clients {
        handle
            .send(client.seat, ClientMessage::Ready)
            .await
            .unwrap();
    }
    assert!(handle.status().await.unwrap().started);
    for (at, client) in clients.iter_mut().enumerate() {
        let messages = client.drain_all();
        let own = told(&messages);
        assert_ne!(own, Vec::<String>::new());
        assert!(
            own.iter().all(|id| id.starts_with(&format!("s{at}-"))),
            "{own:?}"
        );
        assert!(
            !names_seat(&messages, 1 - at),
            "seat {at} was told a hidden card"
        );
    }

    // A reconnect is told the same list with its snapshot.
    let mut again = Client::join(&handle, 0).await;
    let messages = again.drain_all();
    assert!(told(&messages).iter().all(|id| id.starts_with("s0-")));
    assert!(!names_seat(&messages, 1));
}

#[tokio::test]
async fn a_spectator_is_told_no_hidden_deck() {
    let handle = started(2);
    for seat in 0..2 {
        let client = Client::join(&handle, seat).await;
        handle
            .send(client.seat, ClientMessage::Ready)
            .await
            .unwrap();
    }
    assert!(handle.status().await.unwrap().started);
    let mut inbox = handle.watch().await.unwrap();
    let messages: Vec<_> = std::iter::from_fn(|| inbox.try_recv().ok()).collect();
    assert!(
        !messages
            .iter()
            .any(|message| matches!(message, ServerMessage::DeckPictures { .. })),
        "the fuzz decks have no commanders, so nothing is public"
    );
    assert!(!names_seat(&messages, 0) && !names_seat(&messages, 1));
}
