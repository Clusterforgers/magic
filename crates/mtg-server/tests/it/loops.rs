//! Goal: the live case at two to four seats with auto-pass on: Polyraptor beside Aether Flash no longer makes tokens forever but ends the game in a draw by loop for every seat, and with Where Ancients Tread the loop's driver is asked how many times to repeat it, every seat gets the iterations as one summary line in a batch that counts the events it folds, and the next batch follows on from there.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use common::{Client, game_id, roomy};
use mtg_core::color::Color;
use mtg_core::ids::{ObjectId, PlayerId};
use mtg_engine::choose::{Target, TargetChoice, YesNoChoice};
use mtg_engine::decision::{DecisionKind, PayChoice, PriorityAction};
use mtg_engine::loops::LoopAnswer;
use mtg_engine::state::GameResult;
use mtg_engine::{Answer, Event, GameSetup};
use mtg_protocol::{ClientMessage, EventBatch, EventView, ServerMessage};
use mtg_server::{Dwell, GameConfig, GameHandle, MemoryStore, Pace, spawn_game_with};
use mtg_testkit::cards::land;
use mtg_testkit::corpus::card;
use mtg_testkit::{Scenario, ready};
use rstest::rstest;

const ON: Pace = Pace {
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

/// Seat 0 holds Polyraptor and controls `damager` with eight Forests.
fn table(seats: usize, damager: &str) -> GameSetup {
    let mut permanents: Vec<_> = (0..8).map(|_| ready(land(Color::Green))).collect();
    permanents.push(ready(card(damager).unwrap()));
    Scenario::new(seats)
        .hand(0, [card("Polyraptor").unwrap()])
        .battlefield(0, permanents)
        .setup()
        .unwrap()
}

async fn start(seats: usize, damager: &str) -> (GameHandle, Vec<Client>) {
    let config = GameConfig {
        pace: ON,
        ..roomy()
    };
    let handle = spawn_game_with(
        game_id(1),
        table(seats, damager),
        MemoryStore::new(),
        config,
    )
    .unwrap();
    let mut clients = Vec::new();
    for n in 0..seats {
        clients.push(Client::join(&handle, n).await);
    }
    for client in &clients {
        client.say(&handle, ClientMessage::Ready).await;
    }
    handle.status().await.unwrap();
    (handle, clients)
}

/// Seat 0 casts Polyraptor, aims each Where Ancients Tread trigger at the newest Polyraptor and says yes, until the
/// game ends or the loop is offered; returns the offer's decision id.
async fn drive(handle: &GameHandle, seat0: &mut Client) -> Option<mtg_protocol::DecisionId> {
    for _ in 0..200 {
        let message = seat0
            .until(|m| match m {
                ServerMessage::Decision { id, decision, .. } => Some(Some((*id, decision.clone()))),
                ServerMessage::GameOver { .. } => Some(None),
                _ => None,
            })
            .await;
        let (id, decision) = message?;
        let answer = match &decision.kind {
            DecisionKind::Loop(_) => return Some(id),
            DecisionKind::Priority(options) => match options.castable.first() {
                Some(card) => Answer::Priority(PriorityAction::CastSpell(*card)),
                None => Answer::Priority(PriorityAction::Pass),
            },
            DecisionKind::PayMana(_) => Answer::PayMana(PayChoice::Auto),
            DecisionKind::ChooseTargets(options) => {
                let newest = options
                    .legal
                    .iter()
                    .filter(|target| matches!(target, Target::Object(_)))
                    .max_by_key(|target| match target {
                        Target::Object(object) => object.id,
                        Target::Player(_) => ObjectId(0),
                    })
                    .copied()
                    .unwrap();
                Answer::ChooseTargets(TargetChoice::Targets(vec![newest]))
            }
            DecisionKind::YesNo(_) => Answer::YesNo(YesNoChoice(true)),
            _ => common::passive(&decision),
        };
        seat0
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
    None
}

fn batches(messages: &[ServerMessage]) -> Vec<&EventBatch> {
    messages
        .iter()
        .filter_map(|m| match m {
            ServerMessage::Events(batch) => Some(batch),
            _ => None,
        })
        .collect()
}

fn has(batch: &EventBatch, pick: impl Fn(&Event) -> bool) -> bool {
    batch
        .events
        .iter()
        .any(|view| matches!(view, EventView::Public(event) if pick(event)))
}

#[rstest]
#[tokio::test]
async fn a_mandatory_loop_on_an_auto_passing_table_is_a_draw_for_every_seat(
    #[values(2, 3, 4)] seats: usize,
) {
    let (handle, mut clients) = start(seats, "Aether Flash").await;
    assert_eq!(
        drive(&handle, &mut clients[0]).await,
        None,
        "no one is asked"
    );
    let status = handle.status().await.unwrap();
    assert_eq!(status.result, Some(GameResult::Draw));
    for client in &mut clients[1..] {
        let messages = client.drain();
        assert!(
            batches(&messages)
                .iter()
                .any(|batch| has(batch, |e| matches!(e, Event::LoopDrawn { .. })))
        );
        assert!(messages.iter().any(|m| matches!(
            m,
            ServerMessage::GameOver {
                result: GameResult::Draw
            }
        )));
    }
}

#[rstest]
#[tokio::test]
async fn the_driver_chooses_the_iterations_and_every_seat_gets_one_line(
    #[values(2, 3, 4)] seats: usize,
) {
    let (handle, mut clients) = start(seats, "Where Ancients Tread").await;
    let offer = drive(&handle, &mut clients[0])
        .await
        .expect("the loop is offered");
    for client in &mut clients {
        client.drain();
    }
    clients[0]
        .say(
            &handle,
            ClientMessage::Answer {
                decision: offer,
                answer: Answer::Loop(LoopAnswer::Repeat(100)),
            },
        )
        .await;
    handle.status().await.unwrap();
    for client in &mut clients {
        let messages = client.drain();
        let batches = batches(&messages);
        let shortcut = batches
            .iter()
            .position(|batch| {
                has(
                    batch,
                    |e| matches!(e, Event::LoopRepeated(s) if s.iterations == 100),
                )
            })
            .expect("the shortcut's batch");
        let batch = batches[shortcut];
        assert!(batch.folded > 1000, "folded {}", batch.folded);
        assert!(batch.events.len() < 50, "listed {}", batch.events.len());
        if let Some(next) = batches.get(shortcut + 1) {
            let end = batch.from_seq + u64::try_from(batch.events.len()).unwrap() + batch.folded;
            assert_eq!(next.from_seq, end);
        }
    }
    let status = handle.status().await.unwrap();
    assert_eq!(
        status.waiting_on,
        Some(PlayerId(0)),
        "play goes on at the loop's start"
    );
}
