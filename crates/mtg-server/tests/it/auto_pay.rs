//! Goal: "Always pay for me" pays a seat's mana cost at once with no payment prompt, only with the setting on and out of Full Control; the cast it paid is a step like any other, so a take-back asked right after it undoes the cast and the payment together and no payment question is left behind to race it.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use common::{Client, game_id, roomy};
use mtg_engine::choose::{Target, TargetChoice};
use mtg_engine::decision::PriorityAction;
use mtg_engine::{Answer, DecisionKind, GameSetup};
use mtg_protocol::{AutoChoices, ClientMessage, FullControlMode, ServerMessage, TakeBackOutcome};
use mtg_server::{Dwell, GameConfig, GameHandle, MemoryStore, Pace, spawn_game_with};
use mtg_testkit::cards::{burn, free_creature, mountain};
use mtg_testkit::{Scenario, ready};
use rstest::rstest;

/// No clock and no dwell: only the windows and the settings decide who is asked.
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

/// Seat 0 holds a Bolt and a Mountain; seat 1 a Victim.
fn table() -> GameSetup {
    Scenario::new(2)
        .hand(0, [burn("Bolt", 3)])
        .battlefield(0, [ready(mountain())])
        .battlefield(1, [ready(free_creature("Victim", 1, 1))])
        .setup()
        .unwrap()
}

async fn send(handle: &GameHandle, from: &Client, message: ClientMessage) {
    from.say(handle, message).await;
    handle.status().await.unwrap();
}

/// Both seats join, seat 0 takes `choices` and `mode`, and both are ready.
async fn seated(choices: AutoChoices, mode: FullControlMode) -> (GameHandle, Vec<Client>) {
    let config = GameConfig {
        pace: OFF,
        ..roomy()
    };
    let handle = spawn_game_with(game_id(7), table(), MemoryStore::new(), config).unwrap();
    let mut clients = Vec::new();
    for n in 0..2 {
        clients.push(Client::join(&handle, n).await);
    }
    send(&handle, &clients[0], ClientMessage::AutoChoices { choices }).await;
    send(&handle, &clients[0], ClientMessage::FullControl { mode }).await;
    for client in &clients {
        send(&handle, client, ClientMessage::Ready).await;
    }
    (handle, clients)
}

/// Seat 0 casts its Bolt at seat 1 and names the target; the decisions it is sent after that.
async fn cast_the_bolt(handle: &GameHandle, client: &mut Client) -> Vec<ServerMessage> {
    let (id, decision) = client.decision().await;
    let DecisionKind::Priority(options) = &decision.kind else {
        panic!("seat 0 holds priority, got {decision:?}");
    };
    let cast = Answer::Priority(PriorityAction::CastSpell(options.castable[0]));
    send(
        handle,
        client,
        ClientMessage::Answer {
            decision: id,
            answer: cast,
        },
    )
    .await;
    let (id, decision) = client.decision().await;
    let DecisionKind::ChooseTargets(options) = &decision.kind else {
        panic!("the Bolt asks for its target, got {decision:?}");
    };
    let player = options
        .legal
        .iter()
        .copied()
        .find(|target| matches!(target, Target::Player(_)))
        .unwrap();
    let aim = Answer::ChooseTargets(TargetChoice::Targets(vec![player]));
    send(
        handle,
        client,
        ClientMessage::Answer {
            decision: id,
            answer: aim,
        },
    )
    .await;
    for _ in 0..20 {
        tokio::task::yield_now().await;
    }
    client.drain()
}

/// Whether `messages` hold a payment question.
fn asks_to_pay(messages: &[ServerMessage]) -> bool {
    messages.iter().any(|message| {
        matches!(message, ServerMessage::Decision { decision, .. } if matches!(decision.kind, DecisionKind::PayMana(_)))
    })
}

#[rstest]
#[case::off_asks(false, FullControlMode::Off, true)]
#[case::on_pays_at_once(true, FullControlMode::Off, false)]
#[case::full_control_asks_anyway(true, FullControlMode::Held, true)]
#[tokio::test(start_paused = true)]
async fn always_pay_automatically_skips_the_payment_prompt(
    #[case] on: bool,
    #[case] mode: FullControlMode,
    #[case] asked: bool,
) {
    let choices = AutoChoices {
        auto_pay: on,
        ..AutoChoices::default()
    };
    let (handle, mut clients) = seated(choices, mode).await;
    let sent = cast_the_bolt(&handle, &mut clients[0]).await;
    assert_eq!(asks_to_pay(&sent), asked, "{sent:?}");
}

#[tokio::test(start_paused = true)]
async fn a_take_back_after_an_automatic_payment_undoes_the_cast_and_its_payment() {
    let choices = AutoChoices {
        auto_pay: true,
        ..AutoChoices::default()
    };
    let (handle, mut clients) = seated(choices, FullControlMode::Off).await;
    // Seat 1 holds priority over the Bolt, so the cast is still the last thing seat 0 did.
    let held = FullControlMode::Held;
    send(
        &handle,
        &clients[1],
        ClientMessage::FullControl { mode: held },
    )
    .await;
    let sent = cast_the_bolt(&handle, &mut clients[0]).await;
    assert!(!asks_to_pay(&sent), "paid at once");
    send(&handle, &clients[0], ClientMessage::TakeBack).await;
    clients[1]
        .until(|message| matches!(message, ServerMessage::TakeBackAsked { .. }).then_some(()))
        .await;
    send(
        &handle,
        &clients[1],
        ClientMessage::TakeBackVote { approve: true },
    )
    .await;
    // The take-back is a step with its passes; its end comes after the decision it leaves.
    let mut last = None;
    let outcome = loop {
        match clients[0].next().await {
            ServerMessage::Decision { decision, .. } => last = Some(decision),
            ServerMessage::TakeBackEnded { outcome, .. } => break outcome,
            _ => {}
        }
    };
    assert_eq!(outcome, TakeBackOutcome::Approved);
    // Back at priority with the Bolt in hand and nothing to pay: the cast is undone whole.
    let decision = last.expect("seat 0 is asked again");
    let DecisionKind::Priority(options) = &decision.kind else {
        panic!("seat 0 is back at priority, got {decision:?}");
    };
    assert_eq!(options.castable.len(), 1, "the Bolt can be cast again");
}
