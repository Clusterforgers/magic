//! Goal: a seat that yields to an ability on the stack is passed at once there and never asked again while one like it is on top, until it takes the yield back or turns Full Control on; it is told the whole list at each change, and anything but an ability on the stack is refused.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use common::{Client, game_id, roomy};
use mtg_core::ids::{ObjectId, PlayerId};
use mtg_engine::choose::{Target, TargetChoice};
use mtg_engine::decision::{DecisionKind, PayChoice, PriorityAction};
use mtg_engine::{Answer, GameSetup};
use mtg_protocol::{AutoYield, ClientMessage, ErrorCode, FullControlMode, ServerMessage};
use mtg_server::{Dwell, GameConfig, GameHandle, MemoryStore, Pace, spawn_game_with};
use mtg_testkit::cards::{instant, pinger};
use mtg_testkit::{Scenario, ready};
use rstest::rstest;

/// No clock and no dwell: only the windows decide who is asked.
const OFF: Pace = Pace {
    auto_pass: true,
    response: None,
    blocks: None,
    think: None,
    active: None,
    extension: None,
    grace: std::time::Duration::ZERO,
    dwell: Dwell::OFF,
};

const FOE: usize = 0;
const ME: usize = 1;

/// Seat 0, in its first main phase, holds three Pingers (one key between them) and a free instant; seat 1 holds a free
/// instant, so an ability of seat 0 on the stack is a default window it is asked at.
fn pinger_duel() -> GameSetup {
    let pingers = [1, 2, 3].map(|_| ready(pinger("Pinger", 1)));
    Scenario::new(2)
        .battlefield(0, pingers)
        .hand(0, [instant("Foe Zap", 0, &[])])
        .hand(1, [instant("My Zap", 0, &[])])
        .setup()
        .unwrap()
}

/// Joins both seats, puts seat 0 in Full Control so a test drives it at every window, and readies them.
async fn table() -> (GameHandle, Vec<Client>) {
    let config = GameConfig {
        pace: OFF,
        ..roomy()
    };
    let handle = spawn_game_with(game_id(1), pinger_duel(), MemoryStore::new(), config).unwrap();
    let mut clients = Vec::new();
    for n in 0..2 {
        clients.push(Client::join(&handle, n).await);
    }
    let held = ClientMessage::FullControl {
        mode: FullControlMode::Held,
    };
    send(&handle, &clients[FOE], held).await;
    for client in &clients {
        send(&handle, client, ClientMessage::Ready).await;
    }
    (handle, clients)
}

async fn send(handle: &GameHandle, from: &Client, message: ClientMessage) {
    from.say(handle, message).await;
    handle.status().await.unwrap();
}

async fn answer(handle: &GameHandle, from: &mut Client, answer: Answer) {
    let (decision, _) = from.decision().await;
    send(handle, from, ClientMessage::Answer { decision, answer }).await;
}

/// The seat the game waits on.
async fn waiting(handle: &GameHandle) -> Option<PlayerId> {
    handle.status().await.unwrap().waiting_on
}

/// Seat 0 plays the first ability or spell it is offered (`spell` picks which) aimed at itself, then passes on it.
async fn foe_plays(handle: &GameHandle, foe: &mut Client, spell: bool) {
    let (id, decision) = foe.decision().await;
    let DecisionKind::Priority(options) = &decision.kind else {
        panic!("expected priority, got {decision:?}");
    };
    let action = if spell {
        PriorityAction::CastSpell(options.castable[0])
    } else {
        PriorityAction::Activate(options.abilities[0])
    };
    let play = ClientMessage::Answer {
        decision: id,
        answer: Answer::Priority(action),
    };
    send(handle, foe, play).await;
    loop {
        let (id, decision) = foe.decision().await;
        let answer =
            match &decision.kind {
                DecisionKind::ChooseTargets(_) => Answer::ChooseTargets(TargetChoice::Targets(
                    vec![Target::Player(PlayerId::from_index(FOE))],
                )),
                DecisionKind::PayMana(_) => Answer::PayMana(PayChoice::Auto),
                DecisionKind::Priority(_) => Answer::Priority(PriorityAction::Pass),
                other => panic!("unexpected {}", other.name()),
            };
        let pass = matches!(answer, Answer::Priority(_));
        send(
            handle,
            foe,
            ClientMessage::Answer {
                decision: id,
                answer,
            },
        )
        .await;
        if pass {
            return;
        }
    }
}

/// The object on top of the stack as seat `client` sees it.
async fn top_of_stack(handle: &GameHandle, client: &mut Client) -> ObjectId {
    client.drain();
    send(handle, client, ClientMessage::Resync).await;
    client
        .until(|message| match message {
            ServerMessage::Snapshot { state, .. } => state.table.stack.cards.clone(),
            _ => None,
        })
        .await
        .last()
        .copied()
        .unwrap()
}

/// The list in the next `AutoYields` notice seat `client` gets.
async fn told(client: &mut Client) -> Vec<AutoYield> {
    client
        .until(|message| match message {
            ServerMessage::AutoYields { yields } => Some(yields.clone()),
            _ => None,
        })
        .await
}

/// What seat 1 does after it yields to the first Pinger, before the second goes on the stack.
#[derive(Debug, Clone, Copy)]
enum Then {
    Keeps,
    Clears,
    FullControl,
}

#[rstest]
#[case::no_yield(false, Then::Keeps, true)]
#[case::a_yield_passes(true, Then::Keeps, false)]
#[case::a_yield_taken_back_asks(true, Then::Clears, true)]
#[case::full_control_asks(true, Then::FullControl, true)]
#[tokio::test]
async fn a_seat_yielding_to_an_ability_is_passed_at_each_one_like_it(
    #[case] yields: bool,
    #[case] then: Then,
    #[case] asked: bool,
) {
    let (handle, mut clients) = table().await;
    let me = PlayerId::from_index(ME);
    foe_plays(&handle, &mut clients[FOE], false).await;
    assert_eq!(
        waiting(&handle).await,
        Some(me),
        "asked at the first Pinger"
    );
    if yields {
        let object = top_of_stack(&handle, &mut clients[ME]).await;
        send(&handle, &clients[ME], ClientMessage::AutoYield { object }).await;
        let held = told(&mut clients[ME]).await;
        assert_eq!(held.len(), 1);
        assert_eq!(held[0].name, "Pinger");
        // Passed at once: the ability resolved and seat 0 holds priority again.
        assert_eq!(waiting(&handle).await, Some(PlayerId::from_index(FOE)));
        match then {
            Then::Keeps => {}
            Then::Clears => {
                let ability = held[0].clone();
                send(
                    &handle,
                    &clients[ME],
                    ClientMessage::ClearAutoYield { ability },
                )
                .await;
                assert_eq!(told(&mut clients[ME]).await, Vec::new());
            }
            Then::FullControl => {
                let mode = FullControlMode::Held;
                send(&handle, &clients[ME], ClientMessage::FullControl { mode }).await;
            }
        }
    } else {
        answer(
            &handle,
            &mut clients[ME],
            Answer::Priority(PriorityAction::Pass),
        )
        .await;
    }
    clients[ME].drain();
    foe_plays(&handle, &mut clients[FOE], false).await;
    let now = waiting(&handle).await;
    assert_eq!(
        now == Some(me),
        asked,
        "at the second Pinger, waiting on {now:?}"
    );
    let sent = clients[ME]
        .drain()
        .iter()
        .any(|message| matches!(message, ServerMessage::Decision { .. }));
    assert_eq!(sent, asked);
}

#[rstest]
#[case::a_spell(true)]
#[case::a_permanent(false)]
#[tokio::test]
async fn only_an_ability_on_the_stack_can_be_yielded_to(#[case] spell: bool) {
    let (handle, mut clients) = table().await;
    foe_plays(&handle, &mut clients[FOE], spell).await;
    let object = if spell {
        top_of_stack(&handle, &mut clients[ME]).await
    } else {
        ObjectId(1)
    };
    clients[ME].drain();
    send(&handle, &clients[ME], ClientMessage::AutoYield { object }).await;
    let messages = clients[ME].drain();
    assert!(
        matches!(
            messages.as_slice(),
            [ServerMessage::Error {
                code: ErrorCode::BadMessage,
                ..
            }]
        ),
        "{messages:?}"
    );
    assert_eq!(waiting(&handle).await, Some(PlayerId::from_index(ME)));
}
