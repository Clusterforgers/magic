//! Goal: the opening sends every seat its own keep-or-mulligan prompt at once and with no clock whatever the settings; answers arrive in any order without touching another seat's prompt, a seat that does not answer is waited for (also while away) until it answers or concedes, and no prompt shows another seat's hand.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use std::time::Duration;

use common::{Client, FlakyStore, game_id, roomy, table_setup};
use mtg_core::ids::PlayerId;
use mtg_engine::decision::{DecisionKind, MulliganChoice};
use mtg_engine::{Answer, Decision, Event};
use mtg_protocol::{ClientMessage, DecisionId, ServerMessage};
use mtg_server::{Dwell, EventStore, GameConfig, GameHandle, Pace, spawn_game_with};
use rstest::rstest;

const PACE: Pace = Pace {
    auto_pass: true,
    response: Some(Duration::from_secs(10)),
    blocks: Some(Duration::from_secs(45)),
    think: Some(Duration::from_mins(1)),
    active: Some(Duration::from_secs(30)),
    extension: None,
    grace: Duration::ZERO,
    dwell: Dwell::OFF,
};

type Prompt = (DecisionId, Decision, Option<u64>);

async fn start(seats: usize) -> (GameHandle, FlakyStore, Vec<Client>) {
    let store = FlakyStore::new();
    let config = GameConfig {
        pace: PACE,
        ..roomy()
    };
    let handle = spawn_game_with(game_id(1), table_setup(seats, 3), store.clone(), config).unwrap();
    let mut clients = Vec::new();
    for n in 0..seats {
        clients.push(Client::join(&handle, n).await);
    }
    for client in &clients {
        client.say(&handle, ClientMessage::Ready).await;
    }
    handle.status().await.unwrap();
    (handle, store, clients)
}

/// The decisions queued for `client` right now, without waiting.
fn prompts(client: &mut Client) -> Vec<Prompt> {
    client
        .drain()
        .into_iter()
        .filter_map(|m| match m {
            ServerMessage::Decision {
                id,
                decision,
                deadline_ms,
                ..
            } => Some((id, decision, deadline_ms)),
            _ => None,
        })
        .collect()
}

fn hand(prompt: &Prompt) -> Vec<mtg_core::ids::ObjectId> {
    match &prompt.1.kind {
        DecisionKind::Mulligan(options) => options.hand.clone(),
        other => panic!("the opening asked {}", other.name()),
    }
}

async fn answer(handle: &GameHandle, client: &Client, id: DecisionId, choice: MulliganChoice) {
    let answer = Answer::Mulligan(choice);
    client
        .say(
            handle,
            ClientMessage::Answer {
                decision: id,
                answer,
            },
        )
        .await;
    handle.status().await.unwrap();
}

fn answered(log: &[Event]) -> Vec<(PlayerId, bool)> {
    log.iter()
        .filter_map(|e| match e {
            Event::MulliganKept { player } => Some((*player, true)),
            Event::MulliganDeclared { player } => Some((*player, false)),
            _ => None,
        })
        .collect()
}

#[rstest]
#[tokio::test(start_paused = true)]
async fn every_seat_gets_its_own_prompt_at_once_without_a_clock(
    #[values(2, 3, 4, 6)] seats: usize,
) {
    let (_handle, _, mut clients) = start(seats).await;
    let mut hands = Vec::new();
    for client in &mut clients {
        let mine = prompts(client);
        assert_eq!(
            mine.len(),
            1,
            "seat {:?} is asked once, at once",
            client.seat
        );
        assert_eq!(mine[0].1.player, client.seat);
        assert_eq!(mine[0].2, None, "the opening has no clock");
        hands.push(hand(&mine[0]));
    }
    for (at, mine) in hands.iter().enumerate() {
        for (other, theirs) in hands.iter().enumerate() {
            if at != other {
                assert!(
                    mine.iter().all(|card| !theirs.contains(card)),
                    "no prompt holds another seat's card"
                );
            }
        }
    }
}

#[tokio::test(start_paused = true)]
async fn answers_in_any_order_leave_the_other_prompts_as_they_are() {
    let (handle, store, mut clients) = start(4).await;
    let asked: Vec<Prompt> = clients.iter_mut().map(|c| prompts(c).remove(0)).collect();
    // The last seat in seat order answers first, then the others backwards.
    answer(&handle, &clients[3], asked[3].0, MulliganChoice::Mulligan).await;
    for (seat, client) in clients.iter_mut().enumerate().take(3) {
        assert!(
            prompts(client).is_empty(),
            "seat {seat} keeps its prompt and id"
        );
    }
    for seat in (0..3).rev() {
        answer(&handle, &clients[seat], asked[seat].0, MulliganChoice::Keep).await;
    }
    let log = store.inner().load(game_id(1)).await.unwrap();
    let order: Vec<(PlayerId, bool)> = answered(&log);
    assert_eq!(
        order,
        vec![
            (PlayerId(3), false),
            (PlayerId(2), true),
            (PlayerId(1), true),
            (PlayerId(0), true)
        ],
        "the log keeps the order the answers came in"
    );
    let again = prompts(&mut clients[3]);
    assert_eq!(
        again.len(),
        1,
        "the seat that took a mulligan is asked about its new hand"
    );
    assert!(again[0].0 > asked[3].0);
}

/// Lets the actor run for an hour of paused time.
async fn an_hour_passes(handle: &GameHandle) {
    tokio::time::advance(Duration::from_hours(1)).await;
    for _ in 0..20 {
        tokio::task::yield_now().await;
        handle.status().await.unwrap();
    }
}

#[tokio::test(start_paused = true)]
async fn a_seat_that_does_not_answer_is_waited_for() {
    let (handle, store, mut clients) = start(4).await;
    let asked: Vec<Prompt> = clients.iter_mut().map(|c| prompts(c).remove(0)).collect();
    answer(&handle, &clients[1], asked[1].0, MulliganChoice::Mulligan).await;
    let before = handle.status().await.unwrap();
    an_hour_passes(&handle).await;
    let log = store.inner().load(game_id(1)).await.unwrap();
    assert_eq!(
        answered(&log),
        vec![(PlayerId(1), false)],
        "nobody is answered for"
    );
    assert_eq!(handle.status().await.unwrap(), before);
    for client in &mut clients {
        assert!(prompts(client).is_empty(), "no prompt is sent again");
    }
    for seat in [0, 2, 3] {
        answer(&handle, &clients[seat], asked[seat].0, MulliganChoice::Keep).await;
    }
    let again = prompts(&mut clients[1]);
    assert_eq!(
        again.len(),
        1,
        "the mulligan seat is asked about its new hand"
    );
    assert_eq!(again[0].2, None, "still without a clock");
}

#[tokio::test(start_paused = true)]
async fn a_seat_that_leaves_in_the_opening_holds_the_table_until_it_comes_back_and_concedes() {
    let (handle, store, mut clients) = start(3).await;
    let asked: Vec<Prompt> = clients.iter_mut().map(|c| prompts(c).remove(0)).collect();
    handle.leave(PlayerId(2)).await;
    for seat in 0..2 {
        answer(&handle, &clients[seat], asked[seat].0, MulliganChoice::Keep).await;
    }
    an_hour_passes(&handle).await;
    let status = handle.status().await.unwrap();
    assert_eq!(status.waiting_on, Some(PlayerId(2)), "the table waits");
    assert_eq!(status.result, None);

    let mut back = Client::join(&handle, 2).await;
    handle.status().await.unwrap();
    let mine = prompts(&mut back);
    assert_eq!(mine.len(), 1, "the seat is asked again when it comes back");
    assert_eq!(mine[0].0, asked[2].0, "under the same id");
    assert_eq!(mine[0].2, None);
    back.say(&handle, ClientMessage::Concede).await;
    let status = handle.status().await.unwrap();
    assert_eq!(status.result, None, "two players are left");
    assert_ne!(status.waiting_on, Some(PlayerId(2)));
    let log = store.inner().load(game_id(1)).await.unwrap();
    assert!(log.iter().any(|e| matches!(
        e,
        Event::PlayerLost {
            player: PlayerId(2),
            ..
        }
    )));
}
