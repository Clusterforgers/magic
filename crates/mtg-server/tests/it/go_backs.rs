//! Goal: a seat that passed its main phase by hand while the table passed for a bot is offered that step, and `GoBack` cuts the stored log back to its pass with `WentBack` last, sends every seat a `rewound` snapshot whose log names it, and asks the seat at its main phase again; any pass of another person since (sent by hand or given by the table, which would otherwise show where that seat was asked), a table without take-backs or nothing to go back to is refused to the asker alone and changes nothing.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use common::{Client, game_id, poke, roomy, seat, windowed};
use mtg_core::house::HouseRules;
use mtg_engine::decision::{DecisionKind, PriorityAction};
use mtg_engine::go_back::GoBackRefusal;
use mtg_engine::turn::Step;
use mtg_engine::{Answer, Event};
use mtg_protocol::{ClientMessage, DecisionId, EventView, ServerMessage};
use mtg_server::{EventStore, GameConfig, GameHandle, MemoryStore, spawn_game_with};
use mtg_testkit::Scenario;
use mtg_testkit::cards::forest;
use rstest::rstest;

/// Seat 0 holds a Forest on turn 1; both seats are joined and ready, seat 1 marked as a bot's when `bot`.
async fn table(
    config: GameConfig,
    rules: HouseRules,
    bot: bool,
) -> (GameHandle, MemoryStore, Vec<Client>) {
    let setup = Scenario::new(2)
        .hand(0, [forest()])
        .house_rules(rules)
        .setup()
        .unwrap();
    let store = MemoryStore::new();
    let handle = spawn_game_with(game_id(1), setup, store.clone(), config).unwrap();
    let clients = vec![
        Client::join(&handle, 0).await,
        Client::join(&handle, 1).await,
    ];
    if bot {
        handle.mark_bot(seat(1)).await;
    }
    for client in &clients {
        client.say(&handle, ClientMessage::Ready).await;
    }
    handle.status().await.unwrap();
    (handle, store, clients)
}

fn decision_of(messages: &[ServerMessage]) -> Option<(DecisionId, bool)> {
    messages.iter().rev().find_map(|m| match m {
        ServerMessage::Decision { id, decision, .. } => {
            Some((*id, matches!(decision.kind, DecisionKind::Priority(_))))
        }
        _ => None,
    })
}

/// Each offer the messages tell, in order.
fn offers_of(messages: &[ServerMessage]) -> Vec<Option<Step>> {
    messages
        .iter()
        .filter_map(|m| match m {
            ServerMessage::GoBackOffer { offer } => Some(offer.as_ref().map(|offer| offer.step)),
            _ => None,
        })
        .collect()
}

fn pass(id: DecisionId) -> ClientMessage {
    ClientMessage::Answer {
        decision: id,
        answer: Answer::Priority(PriorityAction::Pass),
    }
}

#[tokio::test]
async fn a_seat_goes_back_to_its_main_phase_for_every_seat() {
    let (handle, store, mut clients) = table(windowed(), HouseRules::default(), true).await;
    let first = clients[0].drain();
    clients[1].drain();
    let (id, _) = decision_of(&first).expect("seat 0 is asked in its main phase");
    let before = store.load(game_id(1)).await.unwrap();

    let passed = poke(&handle, &mut clients, 0, pass(id)).await;
    assert_eq!(
        offers_of(&passed[0]),
        [Some(Step::Main1)],
        "{:?}",
        passed[0]
    );
    assert_eq!(
        offers_of(&passed[1]),
        [],
        "the other seat is offered nothing"
    );

    let back = poke(&handle, &mut clients, 0, ClientMessage::GoBack).await;
    for seen in &back {
        let log = seen
            .iter()
            .find_map(|m| match m {
                ServerMessage::Snapshot {
                    rewound: true, log, ..
                } => Some(log),
                _ => None,
            })
            .expect("every seat gets a rewound snapshot");
        assert!(log.iter().any(|e| matches!(
            e,
            EventView::Public(Event::WentBack { player, step: Step::Main1, active })
                if *player == seat(0) && *active == seat(0)
        )));
    }
    assert!(
        decision_of(&back[0]).is_some_and(|(_, priority)| priority),
        "seat 0 is asked again"
    );
    let stored = store.load(game_id(1)).await.unwrap();
    assert_eq!(stored[..before.len()], before[..]);
    assert_eq!(stored.len(), before.len() + 1);
    assert!(matches!(stored.last(), Some(Event::WentBack { .. })));
    assert_eq!(
        offers_of(&back[0]),
        [None],
        "nothing earlier is left to go back to"
    );
}

#[rstest]
#[case::the_other_person_passed_by_hand(
    roomy(),
    HouseRules::default(),
    false,
    GoBackRefusal::OtherPlayerActed
)]
#[case::the_table_passed_for_the_other_person(
    windowed(),
    HouseRules::default(),
    false,
    GoBackRefusal::OtherPlayerActed
)]
#[case::without_take_backs(windowed(), HouseRules { take_backs: Some(false), ..HouseRules::default() }, true, GoBackRefusal::Off)]
#[tokio::test]
async fn a_go_back_the_table_does_not_allow_is_refused_to_the_asker(
    #[case] config: GameConfig,
    #[case] rules: HouseRules,
    #[case] bot: bool,
    #[case] refusal: GoBackRefusal,
) {
    let (handle, store, mut clients) = table(config, rules, bot).await;
    let first = clients[0].drain();
    clients[1].drain();
    let (id, _) = decision_of(&first).expect("seat 0 is asked in its main phase");
    let passed = poke(&handle, &mut clients, 0, pass(id)).await;
    // Where every decision is sent, seat 1 passes by hand.
    if let Some((id, _)) = decision_of(&passed[1]) {
        poke(&handle, &mut clients, 1, pass(id)).await;
    }
    let before = store.load(game_id(1)).await.unwrap();
    let refused = poke(&handle, &mut clients, 0, ClientMessage::GoBack).await;
    assert!(
        refused[0]
            .iter()
            .any(|m| matches!(m, ServerMessage::GoBackRefused { reason } if *reason == refusal)),
        "{:?}",
        refused[0]
    );
    assert!(refused[1].is_empty(), "the other seat hears nothing");
    assert_eq!(store.load(game_id(1)).await.unwrap(), before);
}

#[tokio::test]
async fn nothing_to_go_back_to_is_refused() {
    let (handle, _store, mut clients) = table(windowed(), HouseRules::default(), true).await;
    let refused = poke(&handle, &mut clients, 0, ClientMessage::GoBack).await;
    assert!(refused[0].iter().any(|m| matches!(
        m,
        ServerMessage::GoBackRefused {
            reason: GoBackRefusal::Nothing
        }
    )));
}
