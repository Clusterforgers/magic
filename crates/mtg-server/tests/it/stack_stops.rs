//! Goal: a seat's stack stops decide at the table which spells and abilities on top of the stack it is asked at, for every stack scenario and setting: `All` asks as before, `Opponents` lets its own objects resolve, `Aimed` also lets an opponent's object that targets nothing of the seat's resolve; Full Control asks anyway, a level set at a decision it leaves out passes at once, and the pass comes with no wait whether the seat holds a response or not.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use std::time::Duration;

use common::{Client, game_id, roomy};
use mtg_core::ids::PlayerId;
use mtg_engine::choose::{Target, TargetChoice};
use mtg_engine::decision::{DecisionKind, PayChoice, PriorityAction};
use mtg_engine::{Answer, GameSetup};
use mtg_protocol::{AutoChoices, ClientMessage, FullControlMode, ServerMessage, StackStops};
use mtg_server::{Dwell, GameConfig, GameHandle, MemoryStore, Pace, spawn_game_with};
use mtg_testkit::cards::{instant, pinger};
use mtg_testkit::{Scenario, ready};
use rstest::rstest;
use tokio::time::Instant;

/// No clock; the dwell as given.
fn pace(dwell: Dwell) -> Pace {
    Pace {
        auto_pass: true,
        response: None,
        blocks: None,
        think: None,
        active: None,
        extension: None,
        relaxed: None,
        grace: Duration::ZERO,
        dwell,
    }
}

const FOE: usize = 0;
const ME: usize = 1;

/// Seat 0, in its first main phase, holds three Pingers and a free instant without targets; seat 1 holds `responses`
/// free instants, so with one or more it can act while an object of seat 0 is on the stack.
fn duel(responses: usize) -> GameSetup {
    let pingers = [1, 2, 3].map(|_| ready(pinger("Pinger", 1)));
    Scenario::new(2)
        .battlefield(0, pingers)
        .hand(0, [instant("Foe Zap", 0, &[])])
        .hand(1, (0..responses).map(|_| instant("My Zap", 0, &[])))
        .setup()
        .unwrap()
}

fn holding(level: StackStops) -> AutoChoices {
    AutoChoices {
        stack_stops: level,
        ..AutoChoices::default()
    }
}

/// Joins both seats, puts seat 0 in Full Control so a test drives it at every window, gives seat 1 `level` and readies them.
async fn table(responses: usize, level: StackStops, dwell: Dwell) -> (GameHandle, Vec<Client>) {
    let config = GameConfig {
        pace: pace(dwell),
        ..roomy()
    };
    let setup = duel(responses);
    let handle = spawn_game_with(game_id(1), setup, MemoryStore::new(), config).unwrap();
    let mut clients = Vec::new();
    for n in 0..2 {
        clients.push(Client::join(&handle, n).await);
    }
    let held = ClientMessage::FullControl {
        mode: FullControlMode::Held,
    };
    send(&handle, &clients[FOE], held).await;
    let choices = holding(level);
    send(
        &handle,
        &clients[ME],
        ClientMessage::AutoChoices { choices },
    )
    .await;
    for client in &clients {
        send(&handle, client, ClientMessage::Ready).await;
    }
    (handle, clients)
}

async fn send(handle: &GameHandle, from: &Client, message: ClientMessage) {
    from.say(handle, message).await;
    handle.status().await.unwrap();
}

/// The seat the game waits on.
async fn waiting(handle: &GameHandle) -> Option<PlayerId> {
    handle.status().await.unwrap().waiting_on
}

/// What a seat puts on the stack.
#[derive(Debug, Clone, Copy)]
enum Play {
    /// A Pinger activation aimed at this seat.
    PingAt(usize),
    /// A free instant without targets.
    Zap,
}

/// `client` makes `play` at its priority decision, then passes on it.
async fn plays(handle: &GameHandle, client: &mut Client, play: Play) {
    let (id, decision) = client.decision().await;
    let DecisionKind::Priority(options) = &decision.kind else {
        panic!("expected priority, got {decision:?}");
    };
    let action = match play {
        Play::PingAt(_) => PriorityAction::Activate(options.abilities[0]),
        Play::Zap => PriorityAction::CastSpell(options.castable[0]),
    };
    let answer = Answer::Priority(action);
    let message = ClientMessage::Answer {
        decision: id,
        answer,
    };
    send(handle, client, message).await;
    loop {
        let (id, decision) = client.decision().await;
        let answer = match (&decision.kind, play) {
            (DecisionKind::ChooseTargets(_), Play::PingAt(seat)) => Answer::ChooseTargets(
                TargetChoice::Targets(vec![Target::Player(PlayerId::from_index(seat))]),
            ),
            (DecisionKind::PayMana(_), _) => Answer::PayMana(PayChoice::Auto),
            (DecisionKind::Priority(_), _) => Answer::Priority(PriorityAction::Pass),
            (other, _) => panic!("unexpected {}", other.name()),
        };
        let pass = matches!(answer, Answer::Priority(_));
        let message = ClientMessage::Answer {
            decision: id,
            answer,
        };
        send(handle, client, message).await;
        if pass {
            return;
        }
    }
}

/// Each row: what seat 0 puts on the stack, seat 1's level and Full Control mode, and whether seat 1, which holds a
/// response, is asked there.
#[rstest]
#[case::all_asks_at_an_ability_aimed_at_me(
    Play::PingAt(ME),
    StackStops::All,
    FullControlMode::Off,
    true
)]
#[case::all_asks_at_an_ability_aimed_elsewhere(
    Play::PingAt(FOE),
    StackStops::All,
    FullControlMode::Off,
    true
)]
#[case::all_asks_at_a_spell(Play::Zap, StackStops::All, FullControlMode::Off, true)]
#[case::opponents_asks_at_an_ability_aimed_at_me(
    Play::PingAt(ME),
    StackStops::Opponents,
    FullControlMode::Off,
    true
)]
#[case::opponents_asks_at_an_ability_aimed_elsewhere(
    Play::PingAt(FOE),
    StackStops::Opponents,
    FullControlMode::Off,
    true
)]
#[case::opponents_asks_at_a_spell(Play::Zap, StackStops::Opponents, FullControlMode::Off, true)]
#[case::aimed_asks_at_an_ability_aimed_at_me(
    Play::PingAt(ME),
    StackStops::Aimed,
    FullControlMode::Off,
    true
)]
#[case::aimed_passes_an_ability_aimed_elsewhere(
    Play::PingAt(FOE),
    StackStops::Aimed,
    FullControlMode::Off,
    false
)]
#[case::aimed_passes_a_spell_without_targets(
    Play::Zap,
    StackStops::Aimed,
    FullControlMode::Off,
    false
)]
#[case::full_control_asks_at_an_ability_aimed_elsewhere(
    Play::PingAt(FOE),
    StackStops::Aimed,
    FullControlMode::Held,
    true
)]
#[case::the_next_window_asks_at_a_spell(
    Play::Zap,
    StackStops::Aimed,
    FullControlMode::NextWindow,
    true
)]
#[tokio::test(start_paused = true)]
async fn a_level_asks_the_seat_exactly_at_an_opponents_objects_it_keeps(
    #[case] play: Play,
    #[case] level: StackStops,
    #[case] mode: FullControlMode,
    #[case] asked: bool,
) {
    let (handle, mut clients) = table(1, level, Dwell::OFF).await;
    send(&handle, &clients[ME], ClientMessage::FullControl { mode }).await;
    plays(&handle, &mut clients[FOE], play).await;
    let on = PlayerId::from_index(if asked { ME } else { FOE });
    assert_eq!(waiting(&handle).await, Some(on));
}

/// Seat 1 answers a Pinger aimed at it with an instant of its own and holds one more: only `All` asks it again over
/// its own spell.
#[rstest]
#[case::all_asks_again(StackStops::All, true)]
#[case::opponents_lets_it_resolve(StackStops::Opponents, false)]
#[case::aimed_lets_it_resolve(StackStops::Aimed, false)]
#[tokio::test(start_paused = true)]
async fn a_seats_own_spell_on_top_asks_it_only_at_all(
    #[case] level: StackStops,
    #[case] asked: bool,
) {
    let (handle, mut clients) = table(2, level, Dwell::OFF).await;
    plays(&handle, &mut clients[FOE], Play::PingAt(ME)).await;
    assert_eq!(waiting(&handle).await, Some(PlayerId::from_index(ME)));
    let (id, decision) = clients[ME].decision().await;
    let DecisionKind::Priority(options) = &decision.kind else {
        panic!("expected priority, got {decision:?}");
    };
    let cast = Answer::Priority(PriorityAction::CastSpell(options.castable[0]));
    let message = ClientMessage::Answer {
        decision: id,
        answer: cast,
    };
    send(&handle, &clients[ME], message).await;
    // The cast may ask for its payment; the question after it is priority over the seat's own spell, or none.
    let mut asked_again = false;
    while waiting(&handle).await == Some(PlayerId::from_index(ME)) {
        let (id, decision) = clients[ME].decision().await;
        if matches!(decision.kind, DecisionKind::Priority(_)) {
            asked_again = true;
            break;
        }
        let pay = ClientMessage::Answer {
            decision: id,
            answer: Answer::PayMana(PayChoice::Auto),
        };
        send(&handle, &clients[ME], pay).await;
    }
    assert_eq!(asked_again, asked);
    if !asked {
        assert_eq!(waiting(&handle).await, Some(PlayerId::from_index(FOE)));
    }
}

/// The pass for an object the level leaves out comes at once whatever the seat holds, with the dwell on: a seat with a
/// response and one with an empty hand cannot be told apart by the wait.
#[rstest]
#[case::holding_a_response(1)]
#[case::holding_nothing(0)]
#[tokio::test(start_paused = true)]
async fn the_pass_takes_no_time_whether_the_seat_holds_a_response_or_not(#[case] responses: usize) {
    let (handle, mut clients) = table(responses, StackStops::Aimed, Dwell::DEFAULT).await;
    let since = Instant::now();
    plays(&handle, &mut clients[FOE], Play::PingAt(FOE)).await;
    assert_eq!(waiting(&handle).await, Some(PlayerId::from_index(FOE)));
    assert_eq!(since.elapsed(), Duration::ZERO);
}

#[tokio::test(start_paused = true)]
async fn a_level_set_at_a_decision_it_leaves_out_passes_at_once_and_is_told() {
    let (handle, mut clients) = table(1, StackStops::All, Dwell::OFF).await;
    plays(&handle, &mut clients[FOE], Play::PingAt(FOE)).await;
    assert_eq!(waiting(&handle).await, Some(PlayerId::from_index(ME)));
    clients[ME].drain();
    let choices = holding(StackStops::Aimed);
    send(
        &handle,
        &clients[ME],
        ClientMessage::AutoChoices { choices },
    )
    .await;
    let told = clients[ME]
        .until(|message| match message {
            ServerMessage::AutoChoices { choices } => Some(*choices),
            _ => None,
        })
        .await;
    assert_eq!(told, choices);
    assert_eq!(waiting(&handle).await, Some(PlayerId::from_index(FOE)));

    // A client that joins again is told the level it holds.
    let mut again = Client::join(&handle, ME).await;
    let held = again
        .until(|message| match message {
            ServerMessage::Welcome { auto_choices, .. } => Some(*auto_choices),
            _ => None,
        })
        .await;
    assert_eq!(held.stack_stops, StackStops::Aimed);
}
