//! Goal: in a clocked game a manual action that changes a seat's pending decision issues it again under a new id with the deadline it had, so the clock is neither reset nor extended and the old id is stale; and a manual action sent while a replacement question holds a batch waits, sends nothing, and is applied once the question is answered, after the events it held.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use std::time::Duration;

use common::{Client, game_id, poke, roomy, seat, silent};
use mtg_core::ids::PlayerId;
use mtg_engine::choose::ReplacementChoice;
use mtg_engine::combat::AttackTarget;
use mtg_engine::decision::{DecisionKind, PriorityAction};
use mtg_engine::manual::ManualAction;
use mtg_engine::turn::Step;
use mtg_engine::{Answer, Decision, Event, GameSetup};
use mtg_protocol::{ClientMessage, DecisionId, ErrorCode, FullControlMode, ServerMessage};
use mtg_server::{Dwell, EventStore, GameConfig, GameHandle, MemoryStore, Pace, spawn_game_with};
use mtg_testkit::cards::{forest, free_creature, instant};
use mtg_testkit::corpus::card;
use mtg_testkit::{Scenario, ready};

const RESPONSE: Duration = Duration::from_secs(10);

/// A response clock of 10 s that shows at once, auto-pass on and no dwell.
const CLOCKED: Pace = Pace {
    auto_pass: true,
    response: Some(RESPONSE),
    blocks: Some(Duration::from_secs(45)),
    think: Some(Duration::from_mins(1)),
    active: None,
    extension: None,
    grace: Duration::ZERO,
    dwell: Dwell::OFF,
};

fn spawn(setup: GameSetup) -> (GameHandle, MemoryStore) {
    let store = MemoryStore::new();
    let config = GameConfig {
        pace: CLOCKED,
        ..roomy()
    };
    let handle = spawn_game_with(game_id(1), setup, store.clone(), config).unwrap();
    (handle, store)
}

/// Both seats join and are ready; seat 1 is in Full Control when `held`, so every window of seat 0's turn reaches it.
async fn start(handle: &GameHandle, held: bool) -> Vec<Client> {
    let clients = vec![Client::join(handle, 0).await, Client::join(handle, 1).await];
    if held {
        let mode = FullControlMode::Held;
        clients[1]
            .say(handle, ClientMessage::FullControl { mode })
            .await;
    }
    for client in &clients {
        client.say(handle, ClientMessage::Ready).await;
    }
    handle.status().await.unwrap();
    clients
}

/// The decisions among `messages`, with their deadlines.
fn decisions(messages: &[ServerMessage]) -> Vec<(DecisionId, Decision, Option<u64>)> {
    messages
        .iter()
        .filter_map(|m| match m {
            ServerMessage::Decision {
                id,
                decision,
                deadline_ms,
                ..
            } => Some((*id, decision.clone(), *deadline_ms)),
            _ => None,
        })
        .collect()
}

async fn next_decision(client: &mut Client) -> (DecisionId, Decision, Option<u64>) {
    client
        .until(|m| decisions(std::slice::from_ref(m)).pop())
        .await
}

fn castable(decision: &Decision) -> usize {
    match &decision.kind {
        DecisionKind::Priority(options) => options.castable.len(),
        other => panic!("a priority decision was expected, got {other:?}"),
    }
}

fn passes_by(log: &[Event], player: PlayerId) -> usize {
    log.iter()
        .filter(|e| matches!(e, Event::PriorityPassed { player: p } if *p == player))
        .count()
}

async fn settled(handle: &GameHandle) -> u64 {
    for _ in 0..20 {
        tokio::task::yield_now().await;
        handle.status().await.unwrap();
    }
    handle.status().await.unwrap().events
}

#[tokio::test(start_paused = true)]
async fn a_changed_decision_of_the_same_seat_keeps_its_deadline() {
    // Seat 1 holds nothing and draws a free instant by hand on seat 0's turn.
    let setup = Scenario::new(2)
        .at(1, 0, Step::Main1)
        .library(1, [instant("Zap", 0, &[]), forest(), forest()])
        .setup()
        .unwrap();
    let (handle, store) = spawn(setup);
    let mut clients = start(&handle, true).await;
    let (old, decision, deadline) = next_decision(&mut clients[1]).await;
    assert_eq!(decision.player, seat(1));
    assert_eq!(castable(&decision), 0);
    assert_eq!(deadline, Some(10_000));

    tokio::time::advance(Duration::from_secs(3)).await;
    let draw = ClientMessage::Manual {
        action: ManualAction::Draw { count: 1 },
    };
    let received = poke(&handle, &mut clients, 1, draw).await;
    let sent = decisions(&received[1]);
    let [(new, ref decision, deadline)] = sent[..] else {
        panic!("one new decision for seat 1: {:?}", received[1]);
    };
    assert_ne!(new, old, "the options changed, so the id did");
    assert_eq!(castable(decision), 1);
    assert_eq!(deadline, Some(7_000), "the clock it had, not a new one");
    assert!(decisions(&received[0]).is_empty(), "seat 0 learns nothing");

    let late = ClientMessage::Answer {
        decision: old,
        answer: Answer::Priority(PriorityAction::Pass),
    };
    let received = poke(&handle, &mut clients, 1, late).await;
    assert!(
        received[1].iter().any(|m| matches!(
            m,
            ServerMessage::Error {
                code: ErrorCode::StaleDecision,
                ..
            }
        )),
        "the old id is stale: {:?}",
        received[1]
    );

    let events = settled(&handle).await;
    let passed = passes_by(&store.load(game_id(1)).await.unwrap(), seat(1));
    tokio::time::advance(Duration::from_millis(6_900)).await;
    assert_eq!(settled(&handle).await, events, "the deadline did not move");
    tokio::time::advance(Duration::from_millis(100)).await;
    settled(&handle).await;
    let log = store.load(game_id(1)).await.unwrap();
    assert_eq!(
        passes_by(&log, seat(1)),
        passed + 1,
        "the table passed at 10 s"
    );
}

#[tokio::test(start_paused = true)]
async fn a_manual_action_waits_for_a_replacement_question_and_follows_its_answer() {
    // Two Furnaces of Rath under seat 1 double the damage to seat 1, which chooses the order of the two.
    let setup = Scenario::new(2)
        .at(2, 0, Step::DeclareAttackers)
        .battlefield(0, [ready(free_creature("Raider", 2, 2))])
        .battlefield(
            1,
            [
                ready(card("Furnace of Rath").unwrap()),
                ready(card("Furnace of Rath").unwrap()),
            ],
        )
        .setup()
        .unwrap();
    let (handle, store) = spawn(setup);
    let mut clients = start(&handle, false).await;

    let (attack, decision, _) = next_decision(&mut clients[0]).await;
    let DecisionKind::DeclareAttackers(options) = &decision.kind else {
        panic!("the attack was expected, got {decision:?}");
    };
    let raider = options.attackers[0].object;
    let answer = Answer::DeclareAttackers(vec![(raider, AttackTarget::Player(seat(1)))]);
    clients[0]
        .say(
            &handle,
            ClientMessage::Answer {
                decision: attack,
                answer,
            },
        )
        .await;
    let (question, deadline) = loop {
        let (id, decision, deadline) = next_decision(&mut clients[1]).await;
        let answer = match decision.kind {
            DecisionKind::DeclareBlockers(_) => Answer::DeclareBlockers(Vec::new()),
            DecisionKind::Priority(_) => Answer::Priority(PriorityAction::Pass),
            DecisionKind::ChooseReplacement(_) => break (id, deadline),
            other => panic!("unexpected decision {other:?}"),
        };
        clients[1]
            .say(
                &handle,
                ClientMessage::Answer {
                    decision: id,
                    answer,
                },
            )
            .await;
    };
    assert!(deadline.is_some(), "the question runs on a clock");
    let before = handle.status().await.unwrap();

    let life = ClientMessage::Manual {
        action: ManualAction::Life {
            player: seat(0),
            delta: -1,
        },
    };
    let received = poke(&handle, &mut clients, 0, life).await;
    for messages in &received {
        silent(messages);
    }
    assert_eq!(
        handle.status().await.unwrap(),
        before,
        "nothing applied yet"
    );

    let choose = ClientMessage::Answer {
        decision: question,
        answer: Answer::ChooseReplacement(ReplacementChoice(0)),
    };
    clients[1].say(&handle, choose).await;
    settled(&handle).await;
    let log = store.load(game_id(1)).await.unwrap();
    let damage = log
        .iter()
        .position(|e| matches!(e, Event::DamageDealt { amount: 8, .. }))
        .expect("the doubled damage was dealt");
    let manual = log
        .iter()
        .position(|e| matches!(e, Event::ManualAction { player, .. } if *player == seat(0)))
        .expect("the waiting action was applied");
    assert!(
        damage < manual,
        "the action follows the events it waited for"
    );
    assert_eq!(
        log.iter()
            .filter(|e| matches!(e, Event::ManualAction { .. }))
            .count(),
        1,
        "applied once"
    );
}
