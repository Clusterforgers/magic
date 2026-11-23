//! Goal: "Resolve all" (`Yield { Resolve }`) pressed over a stack of three passes the seat through every object it saw and ends at the empty stack; an object another seat adds stops it with a priority decision; the seat's own choices while things resolve (a discard) are still asked and leave it running, and a trigger of its own going on the stack ends it.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use common::{Client, FlakyStore, game_id, passive, roomy};
use mtg_engine::choose::{Target, TargetChoice, TriggersChoice};
use mtg_engine::decision::{DecisionKind, PayChoice, PriorityAction};
use mtg_engine::{Answer, Decision, GameSetup};
use mtg_protocol::{ClientMessage, DecisionId, FullControlMode, ServerMessage, YieldUntil};
use mtg_server::{Dwell, GameConfig, GameHandle, Pace, spawn_game_with};
use mtg_testkit::cards::{forest, gain_life, instant, mind_rot, swamp};
use mtg_testkit::trigger_cards::{watcher, when};
use mtg_testkit::{Scenario, ready};

/// The default windows decide when seat 1 is asked; no clock and no dwell.
const OFF: Pace = Pace {
    auto_pass: true,
    response: None,
    blocks: None,
    think: None,
    active: None,
    extension: None,
    relaxed: None,
    grace: std::time::Duration::ZERO,
    dwell: Dwell::OFF,
};

const FOE: usize = 0;
const ME: usize = 1;

/// Seat 0, the active player in its first main phase, holds `zaps` free instants; seat 1 holds one, so it can act.
fn zaps(zaps: usize) -> GameSetup {
    let foe = (0..zaps).map(|n| instant(&format!("Foe Zap {n}"), 0, &[]));
    Scenario::new(2)
        .hand(0, foe)
        .hand(1, [instant("My Zap", 0, &[])])
        .setup()
        .unwrap()
}

/// Seat 0 holds Mind Rot (and the swamps for it) under two free instants; seat 1 holds three cards and a creature that
/// triggers whenever it discards.
fn discard_setup() -> GameSetup {
    Scenario::new(2)
        .battlefield(0, [ready(swamp()), ready(swamp())])
        .hand(
            0,
            [
                mind_rot("Mind Rot"),
                instant("Foe Zap 0", 0, &[]),
                instant("Foe Zap 1", 0, &[]),
            ],
        )
        .battlefield(
            1,
            [ready(watcher(
                "Collector",
                when::you_discard(),
                vec![gain_life(1)],
            ))],
        )
        .hand(1, [instant("My Zap", 0, &[]), forest(), forest()])
        .setup()
        .unwrap()
}

/// Seats both clients and readies them; seat 0 is in Full Control, so it is asked at every window.
async fn start(setup: GameSetup) -> (GameHandle, Vec<Client>) {
    let config = GameConfig {
        pace: OFF,
        ..roomy()
    };
    let handle = spawn_game_with(game_id(1), setup, FlakyStore::new(), config).unwrap();
    let clients = vec![
        Client::join(&handle, FOE).await,
        Client::join(&handle, ME).await,
    ];
    let held = ClientMessage::FullControl {
        mode: FullControlMode::Held,
    };
    clients[FOE].say(&handle, held).await;
    for client in &clients {
        client.say(&handle, ClientMessage::Ready).await;
    }
    handle.status().await.unwrap();
    (handle, clients)
}

async fn send(handle: &GameHandle, from: &Client, message: ClientMessage) {
    from.say(handle, message).await;
    handle.status().await.unwrap();
}

async fn answer(handle: &GameHandle, from: &Client, id: DecisionId, answer: Answer) {
    send(
        handle,
        from,
        ClientMessage::Answer {
            decision: id,
            answer,
        },
    )
    .await;
}

/// Seat 0 casts the first spell it can, aimed at seat 1, and pays; it holds priority again afterwards.
async fn foe_casts(handle: &GameHandle, foe: &mut Client) {
    let (id, decision) = foe.decision().await;
    let DecisionKind::Priority(options) = &decision.kind else {
        panic!("expected priority, got {decision:?}");
    };
    let cast = Answer::Priority(PriorityAction::CastSpell(options.castable[0]));
    answer(handle, foe, id, cast).await;
    // Mind Rot asks for its target first; the payment comes last.
    loop {
        let (id, decision) = foe.decision().await;
        match &decision.kind {
            DecisionKind::ChooseTargets(_) => {
                let aim = TargetChoice::Targets(vec![Target::Player(common::seat(ME))]);
                answer(handle, foe, id, Answer::ChooseTargets(aim)).await;
            }
            DecisionKind::PayMana(_) => {
                answer(handle, foe, id, Answer::PayMana(PayChoice::Auto)).await;
                return;
            }
            other => panic!("unexpected {}", other.name()),
        }
    }
}

/// What seat 1 was sent until the game next waits on it with a decision (returned) or, with `None`, until seat 0's
/// passes reach `limit` windows of seat 0.
struct Run {
    told: Vec<ServerMessage>,
    asked: Option<(DecisionId, Decision)>,
}

/// Seat 0 passes at each of its windows (at most `limit`) until the game waits on seat 1, collecting what seat 1 is
/// sent; stops early once `done` holds for what seat 1 was told.
async fn foe_passes(
    handle: &GameHandle,
    clients: &mut [Client],
    limit: usize,
    done: impl Fn(&[ServerMessage]) -> bool,
) -> Run {
    let mut told = Vec::new();
    for _ in 0..limit {
        told.extend(clients[ME].drain());
        if done(&told) {
            break;
        }
        let waiting = handle.status().await.unwrap().waiting_on;
        if waiting == Some(clients[ME].seat) {
            let sent = told
                .iter()
                .rposition(|m| matches!(m, ServerMessage::Decision { .. }));
            if let Some(at) = sent
                && let ServerMessage::Decision { id, decision, .. } = told.remove(at)
            {
                return Run {
                    told,
                    asked: Some((id, decision)),
                };
            }
            let asked = loop {
                match clients[ME].next().await {
                    ServerMessage::Decision { id, decision, .. } => break (id, decision),
                    other => told.push(other),
                }
            };
            return Run {
                told,
                asked: Some(asked),
            };
        }
        let (id, decision) = clients[FOE].decision().await;
        answer(handle, &clients[FOE], id, passive(&decision)).await;
    }
    told.extend(clients[ME].drain());
    Run { told, asked: None }
}

fn notices(messages: &[ServerMessage]) -> Vec<Option<YieldUntil>> {
    messages
        .iter()
        .filter_map(|m| match m {
            ServerMessage::Yielding { until } => Some(*until),
            _ => None,
        })
        .collect()
}

fn ended(messages: &[ServerMessage]) -> bool {
    notices(messages).contains(&None)
}

/// The stack size of the last event batch before the notice that ended the yield.
fn stack_at_end(messages: &[ServerMessage]) -> u32 {
    let end = messages
        .iter()
        .position(|m| matches!(m, ServerMessage::Yielding { until: None }))
        .expect("the yield ended");
    messages[..end]
        .iter()
        .rev()
        .find_map(|m| match m {
            ServerMessage::Events(batch) => Some(batch.table.stack.count),
            _ => None,
        })
        .expect("an event batch before the end")
}

fn priority_decisions(messages: &[ServerMessage]) -> usize {
    messages
        .iter()
        .filter(|m| {
            matches!(m, ServerMessage::Decision { decision, .. } if matches!(decision.kind, DecisionKind::Priority(_)))
        })
        .count()
}

/// Seat 0 casts `count` spells and passes; seat 1 is asked over the stack and presses "Resolve all".
async fn resolve_all_over(handle: &GameHandle, clients: &mut [Client], count: usize) {
    for _ in 0..count {
        foe_casts(handle, &mut clients[FOE]).await;
    }
    let (id, decision) = clients[FOE].decision().await;
    answer(handle, &clients[FOE], id, passive(&decision)).await;
    let (_, decision) = clients[ME].decision().await;
    assert!(matches!(decision.kind, DecisionKind::Priority(_)));
    clients[ME].drain();
    send(
        handle,
        &clients[ME],
        ClientMessage::Yield {
            until: YieldUntil::Resolve,
        },
    )
    .await;
}

#[tokio::test(start_paused = true)]
async fn resolve_all_passes_through_a_stack_of_three_and_ends_at_the_empty_stack() {
    let (handle, mut clients) = start(zaps(3)).await;
    resolve_all_over(&handle, &mut clients, 3).await;

    let run = foe_passes(&handle, &mut clients, 20, ended).await;
    assert!(run.asked.is_none(), "seat 1 was asked {:?}", run.asked);
    assert_eq!(notices(&run.told), [Some(YieldUntil::Resolve), None]);
    assert_eq!(priority_decisions(&run.told), 0);
    assert_eq!(stack_at_end(&run.told), 0, "it ends with the stack empty");
}

#[tokio::test(start_paused = true)]
async fn an_object_another_seat_adds_stops_resolve_all_with_a_priority_decision() {
    let (handle, mut clients) = start(zaps(4)).await;
    resolve_all_over(&handle, &mut clients, 3).await;

    // The seat's pending window is passed for at once; the top spell resolves and seat 0 casts its fourth.
    foe_casts(&handle, &mut clients[FOE]).await;
    let run = foe_passes(&handle, &mut clients, 5, |_| false).await;
    let (_, decision) = run.asked.expect("seat 1 is asked again");
    assert!(matches!(decision.kind, DecisionKind::Priority(_)));
    assert_eq!(notices(&run.told), [Some(YieldUntil::Resolve), None]);
    assert_eq!(stack_at_end(&run.told), 3, "two it saw and the new one");
}

#[tokio::test(start_paused = true)]
async fn the_seats_own_choices_are_asked_and_its_own_trigger_ends_resolve_all() {
    let (handle, mut clients) = start(discard_setup()).await;
    resolve_all_over(&handle, &mut clients, 3).await;

    // Both instants resolve unasked; Mind Rot asks seat 1 which cards it discards, the yield still running.
    let run = foe_passes(&handle, &mut clients, 10, |_| false).await;
    let (id, decision) = run.asked.expect("seat 1 chooses its discards");
    assert!(
        matches!(decision.kind, DecisionKind::ChooseCards(_)),
        "{decision:?}"
    );
    assert_eq!(notices(&run.told), [Some(YieldUntil::Resolve)]);
    assert_eq!(priority_decisions(&run.told), 0);
    answer(&handle, &clients[ME], id, passive(&decision)).await;

    // The discard triggers the Collector: an order of its triggers would be asked with the yield still running, and
    // the trigger on the stack ends the yield.
    let mut told = Vec::new();
    loop {
        let run = foe_passes(&handle, &mut clients, 10, ended).await;
        told.extend(run.told);
        let Some((id, decision)) = run.asked else {
            break;
        };
        assert!(!ended(&told), "the yield ended before {decision:?}");
        let DecisionKind::OrderTriggers(options) = &decision.kind else {
            panic!("seat 1 orders its triggers, got {decision:?}");
        };
        let order = TriggersChoice((0..options.triggers.len()).collect());
        answer(&handle, &clients[ME], id, Answer::OrderTriggers(order)).await;
    }
    assert_eq!(notices(&told), [None]);
    assert!(
        stack_at_end(&told) > 0,
        "it ends with its own trigger on the stack"
    );
}
