//! Goal: every seat hears the same `Deciding` list of who the game waits for: it names the seat the game waits on, each seat once, with the kind of the decision that seat was sent, and a client that rejoins hears it right after its snapshot.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use common::{Client, game_id, roomy, start_table, table_setup};
use mtg_protocol::{DecidingView, DecisionName, ServerMessage};
use mtg_server::{MemoryStore, spawn_game_with};
use rstest::rstest;

/// The last `Deciding` list among `messages`.
fn last_deciding(messages: &[ServerMessage]) -> Option<Vec<DecidingView>> {
    messages.iter().rev().find_map(|message| match message {
        ServerMessage::Deciding { seats } => Some(seats.clone()),
        _ => None,
    })
}

/// The kinds of the decisions `messages` sent their seat, last first.
fn sent_kinds(messages: &[ServerMessage]) -> Vec<DecisionName> {
    messages
        .iter()
        .rev()
        .filter_map(|message| match message {
            ServerMessage::Decision { decision, .. } => Some(DecisionName::of(&decision.kind)),
            _ => None,
        })
        .collect()
}

#[rstest]
#[tokio::test]
async fn every_seat_hears_the_same_seats_with_the_kinds_they_were_sent(
    #[values(2, 3, 4)] players: usize,
    #[values(1, 7)] seed: u64,
) {
    let handle = spawn_game_with(
        game_id(1),
        table_setup(players, seed),
        MemoryStore::new(),
        roomy(),
    )
    .unwrap();
    let (mut clients, first) = start_table(&handle, players).await;
    handle.status().await.unwrap();
    let heard: Vec<Vec<ServerMessage>> = clients.iter_mut().map(Client::drain_all).collect();
    let lists: Vec<Vec<DecidingView>> = heard
        .iter()
        .map(|messages| last_deciding(messages).expect("every seat hears who decides"))
        .collect();
    let list = lists[0].clone();
    assert!(lists.iter().all(|other| *other == list), "{lists:?}");
    assert!(
        list.iter().any(|entry| entry.seat.index() == first),
        "{list:?}"
    );
    for (n, entry) in list.iter().enumerate() {
        assert!(
            list[n + 1..].iter().all(|other| other.seat != entry.seat),
            "{list:?}"
        );
        let own = sent_kinds(&heard[entry.seat.index()]);
        assert_eq!(own.first(), Some(&entry.kind), "seat {:?}", entry.seat);
    }

    // A seat that comes back hears the same list after its snapshot.
    let mut back = Client::join(&handle, 0).await;
    handle.status().await.unwrap();
    let again = back.drain_all();
    let at = again
        .iter()
        .position(|message| matches!(message, ServerMessage::Snapshot { .. }))
        .expect("a snapshot");
    assert!(
        matches!(&again[at + 1], ServerMessage::Deciding { seats } if *seats == list),
        "{again:?}"
    );
}
