//! Goal: a beginning-of-combat trigger (Zndrsplt and Okaun) played through the actor under the default windows, its controller passing or yielding to the end of the turn or to its next turn, flips in the beginning of combat step of its controller's turns in the log every seat is sent.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use common::{Client, game_id, seat, windowed};
use mtg_engine::DecisionKind;
use mtg_engine::Event;
use mtg_engine::turn::Step;
use mtg_protocol::{ClientMessage, EventView, ServerMessage, YieldUntil};
use mtg_server::{MemoryStore, spawn_game_with};
use mtg_testkit::cards::island;
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ready};
use rstest::rstest;

const ZNDRSPLT: &str = "Zndrsplt, Eye of Wisdom";
const OKAUN: &str = "Okaun, Eye of Chaos";

#[rstest]
#[tokio::test]
async fn coin_flips_at_the_beginning_of_combat_land_in_that_step_of_the_log(
    #[values(2, 3)] seats: usize,
    #[values(0, 1)] owner: usize,
    #[values(None, Some(YieldUntil::EndOfTurn), Some(YieldUntil::MyTurn))] yielding: Option<
        YieldUntil,
    >,
) {
    let mut scenario = Scenario::new(seats).battlefield(
        owner,
        [ready(card(ZNDRSPLT).unwrap()), ready(card(OKAUN).unwrap())],
    );
    for each in 0..seats {
        scenario = scenario.library(each, (0..40).map(|_| island()).collect::<Vec<_>>());
    }
    let handle = spawn_game_with(
        game_id(1),
        scenario.setup().unwrap(),
        MemoryStore::new(),
        windowed(),
    )
    .unwrap();
    let mut clients = Vec::new();
    for n in 0..seats {
        clients.push(Client::join(&handle, n).await);
    }
    for client in &clients {
        client.say(&handle, ClientMessage::Ready).await;
    }
    let rounds = 2 * seats;
    let observer = (owner + 1) % seats;
    let mut log: Vec<Event> = Vec::new();
    let mut pending = vec![None; seats];
    for _ in 0..3000 {
        let status = handle.status().await.unwrap();
        for (n, client) in clients.iter_mut().enumerate() {
            for message in client.drain() {
                match message {
                    ServerMessage::Events(batch) if n == observer => {
                        log.extend(batch.events.into_iter().filter_map(|event| match event {
                            EventView::Public(event) => Some(event),
                            EventView::Hidden(_) => None,
                        }));
                    }
                    ServerMessage::Decision { id, decision, .. } => {
                        pending[n] = Some((id, decision));
                    }
                    _ => {}
                }
            }
        }
        let turns = log
            .iter()
            .filter(|event| matches!(event, Event::TurnBegan { .. }))
            .count();
        let Some(waiting) = status.waiting_on else {
            break;
        };
        if turns > rounds {
            break;
        }
        let Some((id, decision)) = pending[waiting.index()].take() else {
            continue;
        };
        if let Some(until) = yielding
            && waiting == seat(owner)
            && matches!(decision.kind, DecisionKind::Priority(_))
        {
            clients[owner]
                .say(&handle, ClientMessage::Yield { until })
                .await;
            continue;
        }
        let answer = Driver::passive_answer(&decision);
        clients[waiting.index()]
            .say(
                &handle,
                ClientMessage::Answer {
                    decision: id,
                    answer,
                },
            )
            .await;
    }
    let mut step = None;
    let mut active = Some(seat(0));
    let mut flips = Vec::new();
    for event in &log {
        match event {
            Event::StepBegan { step: began } => step = Some(*began),
            Event::TurnBegan { active: player, .. } => active = Some(*player),
            Event::CoinFlipped { .. } => flips.push((active, step)),
            _ => {}
        }
    }
    assert!(!flips.is_empty(), "the flips are played");
    for flip in flips {
        assert_eq!(flip, (Some(seat(owner)), Some(Step::BeginCombat)));
    }
}
