//! Goal: stale, wrong-seat, illegal and premature client messages are refused with the right error to the sender alone, leave the game exactly as it was and broadcast nothing; the pending decision stays answerable.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use common::{
    Client, game_id, passive, passive_until_priority, poke, roomy, seat, silent, start_table,
    table_setup,
};
use mtg_core::ids::ObjectId;
use mtg_engine::Answer;
use mtg_engine::Event;
use mtg_engine::decision::{MulliganChoice, PriorityAction};
use mtg_engine::manual::ManualAction;
use mtg_protocol::{ClientMessage, DecisionId, ErrorCode, EventView, ServerMessage};
use mtg_server::{EventStore, GameHandle, MemoryStore, spawn_game_with};
use rstest::rstest;

fn new_game(players: usize) -> (GameHandle, MemoryStore) {
    let store = MemoryStore::new();
    let setup = table_setup(players, 5);
    let handle = spawn_game_with(game_id(1), setup, store.clone(), roomy()).unwrap();
    (handle, store)
}

fn only_error(messages: &[ServerMessage]) -> ErrorCode {
    match messages {
        [ServerMessage::Error { code, .. }] => *code,
        other => panic!("expected exactly one error, got {other:?}"),
    }
}

#[derive(Debug, Clone, Copy)]
enum Bad {
    /// Someone else answers the pending decision.
    WrongSeat,
    /// The holder echoes an id that is not the pending one.
    Id(u64),
    /// The holder answers a mulligan question with an answer of another kind.
    WrongKind,
}

#[rstest]
#[case::wrong_seat(Bad::WrongSeat, ErrorCode::StaleDecision)]
#[case::id_zero(Bad::Id(0), ErrorCode::StaleDecision)]
#[case::next_id(Bad::Id(2), ErrorCode::StaleDecision)]
#[case::far_future_id(Bad::Id(u64::MAX), ErrorCode::StaleDecision)]
#[case::wrong_kind(Bad::WrongKind, ErrorCode::IllegalAnswer)]
#[tokio::test]
async fn a_refused_answer_changes_nothing_and_the_decision_stays_open(
    #[case] bad: Bad,
    #[case] expected: ErrorCode,
) {
    let (handle, store) = new_game(2);
    let (mut clients, holder) = start_table(&handle, 2).await;
    let other = 1 - holder;
    let (id, decision) = clients[holder].decision().await;
    let before = handle.status().await.unwrap();

    let (from, message) = match bad {
        Bad::WrongSeat => (
            other,
            ClientMessage::Answer {
                decision: id,
                answer: passive(&decision),
            },
        ),
        Bad::Id(n) => (
            holder,
            ClientMessage::Answer {
                decision: DecisionId(n),
                answer: passive(&decision),
            },
        ),
        Bad::WrongKind => (
            holder,
            ClientMessage::Answer {
                decision: id,
                answer: Answer::Discard(Vec::new()),
            },
        ),
    };
    let received = poke(&handle, &mut clients, from, message).await;

    assert_eq!(only_error(&received[from]), expected);
    silent(&received[1 - from]);
    assert_eq!(handle.status().await.unwrap(), before);
    assert_eq!(
        store.load(game_id(1)).await.unwrap().len() as u64,
        before.events
    );

    // The same decision can still be answered, with the same id.
    let answer = Answer::Mulligan(MulliganChoice::Keep);
    let received = poke(
        &handle,
        &mut clients,
        holder,
        ClientMessage::Answer {
            decision: id,
            answer,
        },
    )
    .await;
    assert!(matches!(
        received[holder].first(),
        Some(ServerMessage::Events(_))
    ));
    assert!(handle.status().await.unwrap().events > before.events);
}

#[tokio::test]
async fn an_option_that_was_not_offered_is_illegal() {
    let (handle, store) = new_game(2);
    let (mut clients, _) = start_table(&handle, 2).await;
    let (holder, id, _) = passive_until_priority(&handle, &mut clients).await;
    let before = handle.status().await.unwrap();

    let bogus = Answer::Priority(PriorityAction::PlayLand(ObjectId(u64::MAX)));
    let message = ClientMessage::Answer {
        decision: id,
        answer: bogus,
    };
    let received = poke(&handle, &mut clients, holder, message).await;

    assert_eq!(only_error(&received[holder]), ErrorCode::IllegalAnswer);
    silent(&received[1 - holder]);
    assert_eq!(handle.status().await.unwrap(), before);
    assert_eq!(
        store.load(game_id(1)).await.unwrap().len() as u64,
        before.events
    );

    let pass = ClientMessage::Answer {
        decision: id,
        answer: Answer::Priority(PriorityAction::Pass),
    };
    poke(&handle, &mut clients, holder, pass).await;
    assert!(handle.status().await.unwrap().events > before.events);
}

#[tokio::test]
async fn a_decision_is_answered_once_and_the_next_one_goes_to_its_seat_only() {
    let (handle, _) = new_game(3);
    let (mut clients, _) = start_table(&handle, 3).await;
    let (holder, id, decision) = passive_until_priority(&handle, &mut clients).await;

    let answer = passive(&decision);
    let first = ClientMessage::Answer {
        decision: id,
        answer: answer.clone(),
    };
    let received = poke(&handle, &mut clients, holder, first).await;
    let next = handle.status().await.unwrap().waiting_on.unwrap().index();
    for (seat_index, messages) in received.iter().enumerate() {
        let decisions: Vec<DecisionId> = messages
            .iter()
            .filter_map(|m| match m {
                ServerMessage::Decision { id, .. } => Some(*id),
                _ => None,
            })
            .collect();
        if seat_index == next {
            assert_eq!(decisions.len(), 1);
            assert!(decisions[0] > id, "a new decision has a fresh id");
        } else {
            assert_eq!(decisions, [], "{seat_index} must not see {next}'s decision");
        }
        assert!(matches!(messages.first(), Some(ServerMessage::Events(_))));
    }

    // Answering the old decision again is refused: the double click of a nervous player.
    let again = ClientMessage::Answer {
        decision: id,
        answer,
    };
    let received = poke(&handle, &mut clients, holder, again).await;
    assert_eq!(only_error(&received[holder]), ErrorCode::StaleDecision);
}

#[tokio::test]
async fn game_messages_before_the_start_are_refused_and_joining_twice_is_a_bad_message() {
    let (handle, _) = new_game(2);
    let mut clients = vec![
        Client::join(&handle, 0).await,
        Client::join(&handle, 1).await,
    ];
    for client in &mut clients {
        client.drain();
    }
    let early = [
        ClientMessage::Answer {
            decision: DecisionId(1),
            answer: Answer::Priority(PriorityAction::Pass),
        },
        ClientMessage::Concede,
        ClientMessage::Resync,
        ClientMessage::Join {
            protocol_version: mtg_protocol::PROTOCOL_VERSION,
            game: game_id(1).to_string(),
            name: "late".to_owned(),
        },
    ];
    for message in early {
        let received = poke(&handle, &mut clients, 0, message).await;
        assert_eq!(only_error(&received[0]), ErrorCode::BadMessage);
        silent(&received[1]);
    }
    let status = handle.status().await.unwrap();
    assert!(!status.started);
    assert_eq!(status.events, 0);

    // Once the game runs, a join message through the handle is still a bad message.
    for client in &clients {
        client.say(&handle, ClientMessage::Ready).await;
    }
    handle.status().await.unwrap();
    let join = ClientMessage::Join {
        protocol_version: mtg_protocol::PROTOCOL_VERSION,
        game: game_id(1).to_string(),
        name: "again".to_owned(),
    };
    let received = poke(&handle, &mut clients, 1, join).await;
    assert_eq!(only_error(&received[1]), ErrorCode::BadMessage);
    silent(&received[0]);
}

#[tokio::test]
async fn a_message_from_a_seat_that_does_not_exist_is_ignored() {
    let (handle, _) = new_game(2);
    let (mut clients, _) = start_table(&handle, 2).await;
    for client in &mut clients {
        client.drain();
    }
    let before = handle.status().await.unwrap();
    handle.send(seat(9), ClientMessage::Concede).await.unwrap();
    assert_eq!(handle.status().await.unwrap(), before);
    assert!(clients.iter_mut().all(|c| c.drain().is_empty()));
}

#[tokio::test]
async fn a_manual_action_of_a_seat_without_priority_or_one_the_state_forbids_is_refused_and_a_legal_one_reaches_every_seat()
 {
    let (handle, store) = new_game(2);
    let (mut clients, _) = start_table(&handle, 2).await;
    let (holder, id, _) = passive_until_priority(&handle, &mut clients).await;
    let other = 1 - holder;
    let before = handle.status().await.unwrap();
    let life = |player: usize, delta: i32| {
        Answer::Priority(PriorityAction::Manual(ManualAction::Life {
            player: mtg_core::ids::PlayerId::from_index(player),
            delta,
        }))
    };

    // The seat that does not hold priority is refused, whatever it names.
    let message = ClientMessage::Answer {
        decision: id,
        answer: life(other, -1),
    };
    let received = poke(&handle, &mut clients, other, message).await;
    assert_eq!(only_error(&received[other]), ErrorCode::StaleDecision);
    // The holder may not change another seat's life.
    let message = ClientMessage::Answer {
        decision: id,
        answer: life(other, -1),
    };
    let received = poke(&handle, &mut clients, holder, message).await;
    assert_eq!(only_error(&received[holder]), ErrorCode::IllegalAnswer);
    silent(&received[other]);
    assert_eq!(handle.status().await.unwrap(), before);
    assert_eq!(
        store.load(game_id(1)).await.unwrap().len() as u64,
        before.events
    );

    // Its own life is changed and every seat is told, the action first.
    let message = ClientMessage::Answer {
        decision: id,
        answer: life(holder, -3),
    };
    let received = poke(&handle, &mut clients, holder, message).await;
    for messages in &received {
        let Some(ServerMessage::Events(batch)) = messages.first() else {
            panic!("a batch comes first: {messages:?}");
        };
        assert!(matches!(
            batch.events.first(),
            Some(EventView::Public(Event::ManualAction { .. }))
        ));
        assert_eq!(batch.table.players[holder].life, 17);
    }
    assert!(handle.status().await.unwrap().events > before.events);
}
