//! Goal: a seat that is not the active player has a response clock on every priority decision (even with nothing to do), a defender has a longer one for blocking, and nothing else is hurried; at the deadline the server answers for the seat (a pass, or no blocks) through the normal path so a late answer is stale, a reconnect is told the time that is left, the clock runs while the seat is away, and a failing store neither loses the move nor makes the server spin.
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::time::Duration;

use common::{Client, FlakyStore, game_id, roomy, seat, silent, table_setup};
use mtg_core::ids::PlayerId;
use mtg_engine::combat::AttackTarget;
use mtg_engine::decision::{DecisionKind, PriorityAction};
use mtg_engine::turn::Step;
use mtg_engine::{Answer, Decision, Event, GameSetup};
use mtg_protocol::{ClientMessage, DecisionId, ErrorCode, ServerMessage};
use mtg_server::{
    EventStore, GameConfig, GameHandle, GameStatus, Pace, resume_game_with, spawn_game_with,
};
use mtg_testkit::cards::{forest, free_creature, instant};
use mtg_testkit::{Scenario, ready};
use rstest::rstest;

const RESPONSE: Duration = Duration::from_secs(10);
const BLOCKS: Duration = Duration::from_secs(45);

/// The server defaults: auto-pass on, 10 s to respond, 45 s to block.
const DEFAULT: Pace = Pace {
    auto_pass: true,
    response: Some(RESPONSE),
    blocks: Some(BLOCKS),
};

fn config(pace: Pace) -> GameConfig {
    GameConfig { pace, ..roomy() }
}

/// Seat 1 holds a free instant, so it can act in every priority window; seat 0 holds nothing and is passed for.
fn instant_table() -> GameSetup {
    Scenario::new(2)
        .hand(1, [instant("Zap", 0, &[])])
        .setup()
        .unwrap()
}

/// Seat 0 is about to attack with a creature that seat 1 can block.
fn combat_table() -> GameSetup {
    Scenario::new(2)
        .at(2, 0, Step::DeclareAttackers)
        .battlefield(0, [ready(free_creature("Raider", 2, 2))])
        .battlefield(1, [ready(free_creature("Guard", 1, 3))])
        .setup()
        .unwrap()
}

fn spawn(setup: GameSetup, pace: Pace) -> (GameHandle, FlakyStore) {
    let store = FlakyStore::new();
    let handle = spawn_game_with(game_id(1), setup, store.clone(), config(pace)).unwrap();
    (handle, store)
}

async fn join_and_ready(handle: &GameHandle) -> Vec<Client> {
    let mut clients = Vec::new();
    for n in 0..2 {
        clients.push(Client::join(handle, n).await);
    }
    for client in &clients {
        client.say(handle, ClientMessage::Ready).await;
    }
    handle.status().await.unwrap();
    clients
}

/// What a decision message said: its id, content and clock.
type Clocked = (DecisionId, Decision, Option<u64>);

async fn clocked(client: &mut Client) -> Clocked {
    client
        .until(|m| match m {
            ServerMessage::Decision {
                id,
                decision,
                deadline_ms,
            } => Some((*id, decision.clone(), *deadline_ms)),
            _ => None,
        })
        .await
}

/// Lets the actor run until the game has more than `events` events; fails if it never does.
async fn moved_past(handle: &GameHandle, events: u64) -> GameStatus {
    for _ in 0..100 {
        let status = handle.status().await.unwrap();
        if status.events > events {
            return status;
        }
        tokio::task::yield_now().await;
    }
    panic!("the game never got past {events} events");
}

/// Lets the actor run for a while and returns where the game stands.
async fn quiet(handle: &GameHandle) -> GameStatus {
    for _ in 0..20 {
        tokio::task::yield_now().await;
        handle.status().await.unwrap();
    }
    handle.status().await.unwrap()
}

async fn pass(handle: &GameHandle, from: &Client, id: DecisionId) {
    from.say(
        handle,
        ClientMessage::Answer {
            decision: id,
            answer: Answer::Priority(PriorityAction::Pass),
        },
    )
    .await;
}

fn passes_by(log: &[Event], player: PlayerId) -> usize {
    log.iter()
        .filter(|e| matches!(e, Event::PriorityPassed { player: p } if *p == player))
        .count()
}

fn is_stale(messages: &[ServerMessage]) -> bool {
    matches!(
        messages,
        [ServerMessage::Error {
            code: ErrorCode::StaleDecision,
            ..
        }]
    )
}

#[tokio::test(start_paused = true)]
async fn a_seat_that_can_respond_on_someone_elses_turn_has_ten_seconds() {
    let (handle, _) = spawn(instant_table(), DEFAULT);
    let mut clients = join_and_ready(&handle).await;

    let (_, decision, left) = clocked(&mut clients[1]).await;
    assert_eq!(left, Some(10_000));
    let DecisionKind::Priority(options) = &decision.kind else {
        panic!("expected priority, got {decision:?}");
    };
    assert!(options.can_act && options.castable.len() == 1);
    assert_eq!(
        clients[0]
            .drain()
            .iter()
            .filter(|m| matches!(m, ServerMessage::Decision { .. }))
            .count(),
        0
    );
}

#[tokio::test(start_paused = true)]
async fn at_the_deadline_the_server_passes_and_a_late_answer_is_stale() {
    let (handle, store) = spawn(instant_table(), DEFAULT);
    let mut clients = join_and_ready(&handle).await;
    let (id, _, _) = clocked(&mut clients[1]).await;
    let before = handle.status().await.unwrap();
    let passed = passes_by(&store.inner().load(game_id(1)).await.unwrap(), PlayerId(1));
    clients[1].drain();

    tokio::time::advance(Duration::from_secs(9)).await;
    assert_eq!(
        quiet(&handle).await,
        before,
        "a second early, nothing happens"
    );
    silent(&clients[1].drain());

    tokio::time::advance(Duration::from_secs(1)).await;
    let (next, _, left) = clocked(&mut clients[1]).await;
    assert!(next > id, "the next window is a new decision");
    assert_eq!(left, Some(10_000), "with a clock of its own");
    let logged = store.inner().load(game_id(1)).await.unwrap();
    assert_eq!(passes_by(&logged, PlayerId(1)), passed + 1);
    let after = handle.status().await.unwrap();
    assert!(after.events > before.events);

    // The seat's own, late answer meets the existing stale-decision treatment and changes nothing.
    clients[1].drain();
    pass(&handle, &clients[1], id).await;
    assert_eq!(handle.status().await.unwrap(), after);
    assert!(is_stale(&clients[1].drain()));
}

#[tokio::test(start_paused = true)]
async fn an_answer_in_time_stops_the_clock_and_the_next_decision_starts_its_own() {
    let (handle, _) = spawn(instant_table(), DEFAULT);
    let mut clients = join_and_ready(&handle).await;
    let (first, _, _) = clocked(&mut clients[1]).await;

    tokio::time::advance(Duration::from_secs(9)).await;
    pass(&handle, &clients[1], first).await;
    let (second, _, left) = clocked(&mut clients[1]).await;
    assert!(second > first);
    assert_eq!(left, Some(10_000));
    let status = handle.status().await.unwrap();

    // The first clock would have run out now; it was stopped by the answer.
    tokio::time::advance(Duration::from_secs(1)).await;
    assert_eq!(quiet(&handle).await, status);
    silent(&clients[1].drain());

    tokio::time::advance(Duration::from_secs(9)).await;
    let (third, _, _) = clocked(&mut clients[1]).await;
    assert!(third > second, "the second clock ran out on its own time");
}

#[tokio::test(start_paused = true)]
async fn a_pass_made_for_the_seat_is_the_normal_pass_in_the_log() {
    let (handle, store) = spawn(instant_table(), DEFAULT);
    let mut clients = join_and_ready(&handle).await;
    clocked(&mut clients[1]).await;
    let events = handle.status().await.unwrap().events;
    clients[1].drain();

    tokio::time::advance(RESPONSE).await;
    let status = moved_past(&handle, events).await;
    let logged = store.inner().load(game_id(1)).await.unwrap();
    assert_eq!(logged.len() as u64, status.events);
    assert!(matches!(
        logged[usize::try_from(events).unwrap()],
        Event::PriorityPassed { player } if player == PlayerId(1)
    ));
    // Seat 0 saw it like any other pass.
    let seen = clients[0].drain();
    assert!(seen.iter().any(|m| matches!(m, ServerMessage::Events(_))));
}

#[tokio::test(start_paused = true)]
async fn the_active_player_is_never_on_a_clock() {
    let setup = Scenario::new(2).hand(0, [forest()]).setup().unwrap();
    let (handle, _) = spawn(setup, DEFAULT);
    let mut clients = join_and_ready(&handle).await;
    let (_, decision, left) = clocked(&mut clients[0]).await;
    assert_eq!(left, None);
    let DecisionKind::Priority(options) = &decision.kind else {
        panic!("expected priority");
    };
    assert!(options.can_act);

    let status = handle.status().await.unwrap();
    tokio::time::advance(Duration::from_secs(3600)).await;
    assert_eq!(
        quiet(&handle).await,
        status,
        "an hour later the game still waits for seat 0"
    );
    silent(&clients[0].drain());
}

#[tokio::test(start_paused = true)]
async fn a_seat_that_is_not_the_active_player_is_hurried_even_with_nothing_to_do() {
    // Auto-pass off: seat 0's idle window is delivered and, once passed, so is seat 1's.
    let setup = Scenario::new(2).setup().unwrap();
    let (handle, store) = spawn(
        setup,
        Pace {
            auto_pass: false,
            ..DEFAULT
        },
    );
    let mut clients = join_and_ready(&handle).await;
    let (id, _, left) = clocked(&mut clients[0]).await;
    assert_eq!(left, None, "the active player holds its own turn");
    pass(&handle, &clients[0], id).await;
    let (window, decision, left) = clocked(&mut clients[1]).await;
    assert_eq!(
        left,
        Some(10_000),
        "an idle window holds the table for nothing, so it has the usual clock"
    );
    let DecisionKind::Priority(options) = &decision.kind else {
        panic!("expected priority");
    };
    assert!(!options.can_act);

    // At the deadline the server passes for it, as for any seat, and a late answer is stale.
    let events = handle.status().await.unwrap().events;
    let passed = passes_by(&store.inner().load(game_id(1)).await.unwrap(), PlayerId(1));
    clients[1].drain();
    tokio::time::advance(RESPONSE).await;
    let status = moved_past(&handle, events).await;
    assert_eq!(status.waiting_on, Some(seat(0)));
    let logged = store.inner().load(game_id(1)).await.unwrap();
    assert_eq!(passes_by(&logged, PlayerId(1)), passed + 1);
    clients[1].drain();
    pass(&handle, &clients[1], window).await;
    handle.status().await.unwrap();
    assert!(is_stale(&clients[1].drain()));
}

/// Passes for whoever is asked about priority until seat 1 is asked to block; returns that decision.
async fn until_blockers(handle: &GameHandle, clients: &mut [Client]) -> Clocked {
    for _ in 0..50 {
        handle.status().await.unwrap();
        let mut asked = Vec::new();
        for (n, client) in clients.iter_mut().enumerate() {
            asked.extend(client.drain().into_iter().map(|message| (n, message)));
        }
        for (n, message) in asked {
            let ServerMessage::Decision {
                id,
                decision,
                deadline_ms,
            } = message
            else {
                continue;
            };
            match decision.kind {
                DecisionKind::DeclareBlockers(_) => return (id, decision, deadline_ms),
                DecisionKind::Priority(_) => pass(handle, &clients[n], id).await,
                _ => {}
            }
        }
    }
    panic!("seat 1 was never asked to block");
}

#[rstest]
#[case::with_auto_pass(DEFAULT)]
#[case::without_it(Pace { auto_pass: false, ..DEFAULT })]
#[tokio::test(start_paused = true)]
async fn a_defender_has_forty_five_seconds_and_the_expiry_declares_no_blocks(#[case] pace: Pace) {
    let (handle, store) = spawn(combat_table(), pace);
    let mut clients = join_and_ready(&handle).await;

    // The attacker is the active player: no clock for its declaration.
    let (attack_id, decision, left) = clocked(&mut clients[0]).await;
    assert_eq!(left, None);
    let DecisionKind::DeclareAttackers(options) = &decision.kind else {
        panic!("expected the attack, got {decision:?}");
    };
    let raider = options.attackers[0].object;
    let attack = Answer::DeclareAttackers(vec![(raider, AttackTarget::Player(PlayerId(1)))]);
    clients[0]
        .say(
            &handle,
            ClientMessage::Answer {
                decision: attack_id,
                answer: attack,
            },
        )
        .await;

    let (blockers_id, decision, left) = until_blockers(&handle, &mut clients).await;
    assert!(matches!(decision.kind, DecisionKind::DeclareBlockers(_)));
    assert_eq!(left, Some(45_000));
    let events = handle.status().await.unwrap().events;

    tokio::time::advance(Duration::from_secs(44)).await;
    assert_eq!(quiet(&handle).await.events, events, "a second early");
    tokio::time::advance(Duration::from_secs(1)).await;
    moved_past(&handle, events).await;

    let logged = store.inner().load(game_id(1)).await.unwrap();
    assert!(logged.iter().any(|e| matches!(
        e,
        Event::BlockersDeclared { player, blocks } if *player == PlayerId(1) && blocks.is_empty()
    )));
    clients[1].drain();
    clients[1]
        .say(
            &handle,
            ClientMessage::Answer {
                decision: blockers_id,
                answer: Answer::DeclareBlockers(Vec::new()),
            },
        )
        .await;
    handle.status().await.unwrap();
    assert!(is_stale(&clients[1].drain()));
}

#[tokio::test(start_paused = true)]
async fn a_reconnect_is_sent_the_time_that_is_left() {
    let (handle, _) = spawn(instant_table(), DEFAULT);
    let mut clients = join_and_ready(&handle).await;
    let (id, decision, left) = clocked(&mut clients[1]).await;
    assert_eq!(left, Some(10_000));

    tokio::time::advance(Duration::from_secs(4)).await;
    let mut back = Client::join(&handle, 1).await;
    assert!(matches!(back.next().await, ServerMessage::Welcome { .. }));
    assert!(matches!(back.next().await, ServerMessage::Snapshot { .. }));
    assert_eq!(
        back.next().await,
        ServerMessage::Decision {
            id,
            decision: decision.clone(),
            deadline_ms: Some(6_000),
        }
    );

    tokio::time::advance(Duration::from_secs(3)).await;
    handle.send(seat(1), ClientMessage::Resync).await.unwrap();
    let (again, _, left) = clocked(&mut back).await;
    assert_eq!(
        (again, left),
        (id, Some(3_000)),
        "the same decision with less time"
    );
}

#[tokio::test(start_paused = true)]
async fn the_clock_runs_while_the_seat_is_away() {
    let (handle, _) = spawn(instant_table(), DEFAULT);
    let mut clients = join_and_ready(&handle).await;
    let (id, _, _) = clocked(&mut clients[1]).await;
    let events = handle.status().await.unwrap().events;

    handle.leave(seat(1)).await;
    tokio::time::advance(RESPONSE).await;
    moved_past(&handle, events).await;

    let mut back = Client::join(&handle, 1).await;
    assert!(matches!(back.next().await, ServerMessage::Welcome { .. }));
    assert!(matches!(back.next().await, ServerMessage::Snapshot { .. }));
    let (next, _, left) = clocked(&mut back).await;
    assert!(next > id, "the window it missed was passed for it");
    assert_eq!(left, Some(10_000));
}

#[tokio::test(start_paused = true)]
async fn clocks_can_be_switched_off() {
    let pace = Pace {
        response: None,
        blocks: None,
        ..DEFAULT
    };
    let (handle, _) = spawn(instant_table(), pace);
    let mut clients = join_and_ready(&handle).await;
    let (_, _, left) = clocked(&mut clients[1]).await;
    assert_eq!(left, None);
    let status = handle.status().await.unwrap();
    tokio::time::advance(Duration::from_secs(3600)).await;
    assert_eq!(quiet(&handle).await, status);
}

#[tokio::test(start_paused = true)]
async fn the_opening_decisions_are_never_hurried() {
    let (handle, _) = spawn(table_setup(3, 5), DEFAULT);
    let mut clients = Vec::new();
    for n in 0..3 {
        clients.push(Client::join(&handle, n).await);
    }
    for client in &clients {
        client.say(&handle, ClientMessage::Ready).await;
    }
    let status = handle.status().await.unwrap();
    let waiting = status.waiting_on.unwrap().index();
    let (_, decision, left) = clocked(&mut clients[waiting]).await;
    assert!(matches!(decision.kind, DecisionKind::Mulligan(_)));
    assert_eq!(left, None);

    tokio::time::advance(Duration::from_secs(3600)).await;
    assert_eq!(quiet(&handle).await, status);
}

#[tokio::test(start_paused = true)]
async fn a_failed_save_at_the_deadline_keeps_the_decision_and_retries_a_second_later() {
    let (handle, store) = spawn(instant_table(), DEFAULT);
    let mut clients = join_and_ready(&handle).await;
    let (id, _, _) = clocked(&mut clients[1]).await;
    let before = handle.status().await.unwrap();
    clients[1].drain();

    store.fail(true);
    tokio::time::advance(RESPONSE).await;
    let error = clients[1]
        .until(|m| match m {
            ServerMessage::Error { code, .. } => Some(*code),
            _ => None,
        })
        .await;
    assert_eq!(error, ErrorCode::Internal);
    assert_eq!(handle.status().await.unwrap(), before, "nothing changed");
    // No tight loop: one error, then nothing until the retry.
    quiet(&handle).await;
    assert_eq!(clients[1].drain().len(), 0);

    tokio::time::advance(Duration::from_secs(1)).await;
    clients[1]
        .until(|m| matches!(m, ServerMessage::Error { .. }).then_some(()))
        .await;

    store.fail(false);
    tokio::time::advance(Duration::from_secs(1)).await;
    let (next, _, left) = clocked(&mut clients[1]).await;
    assert!(next > id);
    assert_eq!(left, Some(10_000));
    assert!(handle.status().await.unwrap().events > before.events);
}

#[tokio::test(start_paused = true)]
async fn a_resumed_game_starts_the_clock_of_its_pending_decision_afresh() {
    let setup = instant_table();
    let store = FlakyStore::new();
    let first = spawn_game_with(game_id(1), setup.clone(), store.clone(), config(DEFAULT)).unwrap();
    let mut clients = join_and_ready(&first).await;
    let (_, _, left) = clocked(&mut clients[1]).await;
    assert_eq!(left, Some(10_000));
    let stopped = first.status().await.unwrap();
    drop(clients);
    drop(first);
    tokio::time::advance(Duration::from_secs(5)).await;

    let second = resume_game_with(store.clone(), game_id(1), setup, config(DEFAULT))
        .await
        .unwrap();
    assert_eq!(second.status().await.unwrap().events, stopped.events);
    let mut back = Client::join(&second, 1).await;
    assert!(matches!(back.next().await, ServerMessage::Welcome { .. }));
    assert!(matches!(back.next().await, ServerMessage::Snapshot { .. }));
    let (resumed_id, _, left) = clocked(&mut back).await;
    assert_eq!(
        left,
        Some(10_000),
        "the process was away; the clock starts over"
    );

    tokio::time::advance(RESPONSE).await;
    let (next, _, _) = clocked(&mut back).await;
    assert!(next > resumed_id, "and it runs out like any other");
}
