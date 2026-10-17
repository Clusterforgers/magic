//! Goal: a concession sent through the handle takes that player out correctly: the table is told, the log records it, the game ends when one player is left, a decision the concession did not change keeps its id, and the stored log replays to the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use common::{
    Bot, Client, drive, game_id, passive, passive_until_priority, play_bots, poke, roomy, seat,
    start_table, table_setup,
};
use mtg_core::ids::PlayerId;
use mtg_engine::player::LossReason;
use mtg_engine::state::GameResult;
use mtg_engine::{Event, replay};
use mtg_protocol::{ClientMessage, DecisionId, ErrorCode, EventView, ServerMessage};
use mtg_server::{EventStore, GameHandle, MemoryStore, spawn_game_with};
use rstest::rstest;

fn new_game(players: usize, seed: u64) -> (GameHandle, MemoryStore) {
    let store = MemoryStore::new();
    let handle = spawn_game_with(
        game_id(1),
        table_setup(players, seed),
        store.clone(),
        roomy(),
    )
    .unwrap();
    (handle, store)
}

/// Who was reported as having lost, and why, in the event batches among `messages`.
fn losses(messages: &[ServerMessage]) -> Vec<(PlayerId, LossReason)> {
    messages
        .iter()
        .filter_map(|m| match m {
            ServerMessage::Events(batch) => Some(&batch.events),
            _ => None,
        })
        .flatten()
        .filter_map(|e| match e {
            EventView::Public(Event::PlayerLost { player, reason }) => {
                Some((*player, reason.clone()))
            }
            _ => None,
        })
        .collect()
}

fn decisions(messages: &[ServerMessage]) -> Vec<DecisionId> {
    messages
        .iter()
        .filter_map(|m| match m {
            ServerMessage::Decision { id, .. } => Some(*id),
            _ => None,
        })
        .collect()
}

#[rstest]
#[tokio::test]
async fn conceding_ends_a_two_player_game_at_once(
    #[values(0, 1, 40, 200)] answers_before: usize,
    #[values(0, 1)] conceder: usize,
) {
    let (handle, store) = new_game(2, 11);
    let mut bots = vec![
        Bot::join(&handle, 0, 11).await,
        Bot::join(&handle, 1, 11).await,
    ];
    for bot in &bots {
        handle.send(bot.seat, ClientMessage::Ready).await.unwrap();
    }
    assert_eq!(drive(&mut bots, &handle, answers_before).await, None);
    assert_eq!(
        bots.iter().map(|b| b.answers).sum::<usize>(),
        answers_before
    );

    handle
        .send(seat(conceder), ClientMessage::Concede)
        .await
        .unwrap();
    let over = drive(&mut bots, &handle, usize::MAX).await;

    let winner = seat(1 - conceder);
    assert_eq!(over, Some(GameResult::Winner(winner)));
    let status = handle.status().await.unwrap();
    assert_eq!(status.result, Some(GameResult::Winner(winner)));
    assert_eq!(status.waiting_on, None);
    let logged = store.load(game_id(1)).await.unwrap();
    assert!(matches!(logged.last(), Some(Event::GameEnded { .. })));
    assert!(logged.contains(&Event::PlayerLost {
        player: seat(conceder),
        reason: LossReason::Conceded
    }));
    let replayed = replay(&table_setup(2, 11), &logged).unwrap();
    assert_eq!(status.state_hash, Some(replayed.state_hash()));
    for bot in &bots {
        assert_eq!(bot.next_seq, logged.len() as u64);
        assert_eq!(bot.errors, []);
    }
}

#[rstest]
#[tokio::test]
async fn a_bot_that_concedes_mid_game_leaves_the_others_to_finish(
    #[values(3, 4)] players: usize,
    #[values(0, 1, 2)] conceder: usize,
    #[values(5, 40)] after_decisions: usize,
) {
    let seed = 13;
    let (handle, store) = new_game(players, seed);
    let mut bots = Vec::new();
    for n in 0..players {
        bots.push(Bot::join(&handle, n, seed).await);
    }
    bots[conceder].concede_at = Some(after_decisions);

    let reports = play_bots(&handle, bots).await;

    let logged = store.load(game_id(1)).await.unwrap();
    let conceded: Vec<_> = logged
        .iter()
        .filter_map(|e| match e {
            Event::PlayerLost {
                player,
                reason: LossReason::Conceded,
            } => Some(*player),
            _ => None,
        })
        .collect();
    assert_eq!(conceded, [seat(conceder)]);
    let status = handle.status().await.unwrap();
    assert_ne!(status.result, Some(GameResult::Winner(seat(conceder))));
    let replayed = replay(&table_setup(players, seed), &logged).unwrap();
    assert_eq!(status.state_hash, Some(replayed.state_hash()));
    for report in &reports {
        assert_eq!(Some(report.result), status.result, "{:?}", report.seat);
        assert_eq!(report.events_seen, logged.len() as u64);
        assert_eq!(report.errors, []);
    }
}

#[tokio::test]
async fn a_concession_elsewhere_does_not_disturb_a_pending_decision() {
    let (handle, store) = new_game(3, 21);
    let (mut clients, _) = start_table(&handle, 3).await;
    let (holder, id, decision) = passive_until_priority(&handle, &mut clients).await;
    let bystander = (holder + 1) % 3;
    let saved = store.load(game_id(1)).await.unwrap().len();

    let received = poke(&handle, &mut clients, bystander, ClientMessage::Concede).await;

    for messages in &received {
        assert_eq!(losses(messages), [(seat(bystander), LossReason::Conceded)]);
    }
    assert_eq!(
        decisions(&received[holder]),
        [],
        "an unchanged decision is not sent again"
    );
    assert!(store.load(game_id(1)).await.unwrap().len() > saved);
    assert_eq!(
        handle.status().await.unwrap().waiting_on,
        Some(seat(holder))
    );

    // The holder's answer to the old decision is still good.
    let pass = ClientMessage::Answer {
        decision: id,
        answer: passive(&decision),
    };
    let received = poke(&handle, &mut clients, holder, pass).await;
    assert!(matches!(
        received[holder].first(),
        Some(ServerMessage::Events(_))
    ));
    assert!(
        !received[holder]
            .iter()
            .any(|m| matches!(m, ServerMessage::Error { .. }))
    );
}

#[tokio::test]
async fn the_holder_conceding_passes_the_decision_on_with_a_fresh_id() {
    let (handle, _) = new_game(3, 21);
    let (mut clients, _) = start_table(&handle, 3).await;
    let (holder, id, decision) = passive_until_priority(&handle, &mut clients).await;

    let received = poke(&handle, &mut clients, holder, ClientMessage::Concede).await;

    for messages in &received {
        assert_eq!(losses(messages), [(seat(holder), LossReason::Conceded)]);
    }
    assert_eq!(
        decisions(&received[holder]),
        [],
        "the conceder is never asked again"
    );
    let next = handle.status().await.unwrap().waiting_on.unwrap().index();
    assert_ne!(next, holder);
    let fresh = decisions(&received[next]);
    assert_eq!(fresh.len(), 1);
    assert!(fresh[0] > id);

    // The old decision is gone for good.
    let late = ClientMessage::Answer {
        decision: id,
        answer: passive(&decision),
    };
    let received = poke(&handle, &mut clients, holder, late).await;
    assert!(matches!(
        received[holder].as_slice(),
        [ServerMessage::Error {
            code: ErrorCode::StaleDecision,
            ..
        }]
    ));
}

#[tokio::test]
async fn conceding_twice_or_after_the_end_is_a_bad_message() {
    let (handle, _) = new_game(3, 2);
    let (mut clients, _) = start_table(&handle, 3).await;
    poke(&handle, &mut clients, 2, ClientMessage::Concede).await;

    let again = poke(&handle, &mut clients, 2, ClientMessage::Concede).await;
    assert!(matches!(
        again[2].as_slice(),
        [ServerMessage::Error {
            code: ErrorCode::BadMessage,
            ..
        }]
    ));
    poke(&handle, &mut clients, 1, ClientMessage::Concede).await;
    assert_eq!(
        handle.status().await.unwrap().result,
        Some(GameResult::Winner(seat(0)))
    );

    let late = poke(&handle, &mut clients, 0, ClientMessage::Concede).await;
    assert!(matches!(
        late[0].as_slice(),
        [ServerMessage::Error {
            code: ErrorCode::BadMessage,
            ..
        }]
    ));
}

#[tokio::test]
async fn a_player_who_conceded_can_come_back_and_watch_the_game_end() {
    let (handle, _) = new_game(3, 2);
    let (mut clients, _) = start_table(&handle, 3).await;
    poke(&handle, &mut clients, 2, ClientMessage::Concede).await;
    handle.leave(seat(2)).await;

    let mut back = Client::join(&handle, 2).await;
    assert!(matches!(back.next().await, ServerMessage::Welcome { .. }));
    let ServerMessage::Snapshot { state, .. } = back.next().await else {
        panic!("expected a snapshot");
    };
    assert_eq!(state.table.players[2].lost, Some(LossReason::Conceded));

    poke(&handle, &mut clients, 1, ClientMessage::Concede).await;
    let ended = back
        .until(|m| match m {
            ServerMessage::GameOver { result } => Some(*result),
            _ => None,
        })
        .await;
    assert_eq!(ended, GameResult::Winner(seat(0)));

    // A late joiner to a finished game is told the result straight away.
    let mut late = Client::join(&handle, 1).await;
    let messages: Vec<_> = vec![late.next().await, late.next().await, late.next().await];
    assert!(matches!(messages[0], ServerMessage::Welcome { .. }));
    assert!(matches!(messages[1], ServerMessage::Snapshot { .. }));
    assert!(matches!(
        messages[2],
        ServerMessage::GameOver {
            result: GameResult::Winner(_)
        }
    ));
}
