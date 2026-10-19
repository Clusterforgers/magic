//! Goal: a seat is asked for priority at the default windows (its own main phases, the declaring steps, an object of another seat on the stack) only if it can act and at its stops whatever it holds; a one-turn stop goes once its step has passed (a lasting one never) and the seat is told after the events and before the next decision; a stop ends a yield; Full Control asks at every window, `NextWindow` ends with the seat's pass (its own or the clock's), a yield is refused in Full Control, and turning Full Control on ends one; auto-pass off is `Held`; a failed save keeps stops and modes; every delivered window of a seat that is not the active player has the same clock; and bot games equal the reference game under the same rule.
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::time::Duration;

use common::{
    Bot, Client, FlakyStore, game_id, passive, play_bots, reference_windows, silent, table_setup,
    windowed,
};
use mtg_engine::decision::{PayChoice, PriorityAction};
use mtg_engine::turn::Step;
use mtg_engine::{Answer, Decision, DecisionKind, GameSetup};
use mtg_protocol::{
    ClientMessage, DecisionId, ErrorCode, FullControlMode, ServerMessage, Stop, StopSide,
    YieldUntil,
};
use mtg_server::{Dwell, EventStore, GameConfig, GameHandle, MemoryStore, Pace, spawn_game_with};
use mtg_testkit::Scenario;
use mtg_testkit::cards::{giant, instant};
use rstest::rstest;

const FOE: usize = 0;
const ME: usize = 1;

/// The server defaults for the clocks: 10 s to respond, 45 s to block.
const CLOCKS: Pace = Pace {
    auto_pass: true,
    response: Some(Duration::from_secs(10)),
    blocks: Some(Duration::from_secs(45)),
    dwell: Dwell::OFF,
};

fn own(step: Step, keep: bool) -> Stop {
    Stop {
        side: StopSide::Own,
        step,
        keep,
    }
}

fn opponent(step: Step, keep: bool) -> Stop {
    Stop {
        side: StopSide::Opponent,
        step,
        keep,
    }
}

fn set(stop: Stop) -> ClientMessage {
    ClientMessage::SetStop {
        side: stop.side,
        step: stop.step,
        on: true,
        keep: stop.keep,
    }
}

fn mode(mode: FullControlMode) -> ClientMessage {
    ClientMessage::FullControl { mode }
}

/// Two seats whose libraries hold nothing they can cast, so a seat can act only with what a scenario puts in its hand.
fn quiet() -> Scenario {
    Scenario::new(2)
        .library(0, vec![giant(); 30])
        .library(1, vec![giant(); 30])
}

/// The turn, the step and the seat that was asked, as the latest batch the seat had been sent left the game.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Ask {
    seat: usize,
    turn: u32,
    step: Step,
}

/// A window the server asked a seat about, with everything the seat was sent since it last answered.
struct Asked {
    ask: Ask,
    id: DecisionId,
    decision: Decision,
    told: Vec<ServerMessage>,
}

/// Hand-driven clients of one game.
struct Table {
    handle: GameHandle,
    store: FlakyStore,
    clients: Vec<Client>,
    /// What each client was sent and nobody has looked at yet.
    unread: Vec<Vec<ServerMessage>>,
    /// Where the latest batch each client was sent left the game.
    at: Vec<Option<(u32, Step)>>,
}

impl Table {
    /// Seats `seats` clients in the lobby of a game made from `setup`.
    async fn join(setup: GameSetup, config: GameConfig, seats: usize) -> Table {
        let store = FlakyStore::new();
        let handle = spawn_game_with(game_id(1), setup, store.clone(), config).unwrap();
        let mut clients = Vec::new();
        for n in 0..seats {
            clients.push(Client::join(&handle, n).await);
        }
        Table {
            handle,
            store,
            clients,
            unread: vec![Vec::new(); seats],
            at: vec![None; seats],
        }
    }

    /// Sends `message` as `seat` and waits until the actor has dealt with it.
    async fn send(&mut self, seat: usize, message: ClientMessage) {
        self.clients[seat].say(&self.handle, message).await;
        self.handle.status().await.unwrap();
    }

    /// Every seat says it is ready; the game starts.
    async fn start(&mut self) {
        for seat in 0..self.clients.len() {
            self.send(seat, ClientMessage::Ready).await;
        }
    }

    /// Moves everything the clients were sent into `unread`, and notes where the game stood.
    fn collect(&mut self) {
        for (n, client) in self.clients.iter_mut().enumerate() {
            let messages = client.drain();
            let latest = messages.iter().rev().find_map(|m| match m {
                ServerMessage::Events(batch) => {
                    Some((batch.table.turn.number, batch.table.turn.step))
                }
                _ => None,
            });
            self.at[n] = latest.or(self.at[n]);
            self.unread[n].extend(messages);
        }
    }

    /// What `seat` was sent since the last time it was looked at.
    fn take(&mut self, seat: usize) -> Vec<ServerMessage> {
        self.collect();
        std::mem::take(&mut self.unread[seat])
    }

    /// The window the game waits at now, or `None` once it has ended.
    async fn window(&mut self) -> Option<Asked> {
        let waiting = self.handle.status().await.unwrap().waiting_on?.index();
        let told = self.take(waiting);
        let (id, decision) = told
            .iter()
            .rev()
            .find_map(|m| match m {
                ServerMessage::Decision { id, decision, .. } => Some((*id, decision.clone())),
                _ => None,
            })
            .expect("the seat the game waits on was sent its decision");
        let (turn, step) = self.at[waiting].expect("a batch came before the decision");
        let ask = Ask {
            seat: waiting,
            turn,
            step,
        };
        Some(Asked {
            ask,
            id,
            decision,
            told,
        })
    }

    /// Answers the window with `answer`.
    async fn answer(&mut self, asked: &Asked, answer: Answer) {
        let message = ClientMessage::Answer {
            decision: asked.id,
            answer,
        };
        self.send(asked.ask.seat, message).await;
    }

    /// Answers the window with the harmless answer (a pass for priority).
    async fn pass(&mut self, asked: &Asked) {
        self.answer(asked, passive(&asked.decision)).await;
    }

    /// Passes at every window until `done` accepts one; returns the windows passed and the one it stopped at, which
    /// is not answered (none if the game ended first).
    async fn run(&mut self, done: impl Fn(&Ask) -> bool) -> (Vec<Ask>, Option<Asked>) {
        let mut asks = Vec::new();
        while let Some(asked) = self.window().await {
            if done(&asked.ask) {
                return (asks, Some(asked));
            }
            asks.push(asked.ask);
            self.pass(&asked).await;
        }
        (asks, None)
    }

    /// Passes at every window until one of turn `turn` or later.
    async fn run_to_turn(&mut self, turn: u32) -> (Vec<Ask>, Option<Asked>) {
        self.run(|ask| ask.turn >= turn).await
    }

    /// Connects `seat` again, as a reconnect does, and returns what the new connection's `Welcome` states.
    ///
    /// The new connection replaces the old one; the snapshot and pending decision it is sent are not news.
    async fn welcome(&mut self, seat: usize) -> Welcomed {
        self.collect();
        let mut client = Client::join(&self.handle, seat).await;
        let welcome = match client.next().await {
            ServerMessage::Welcome {
                auto_pass,
                full_control,
                stops,
                yielding,
                ..
            } => Welcomed {
                auto_pass,
                full_control,
                stops,
                yielding,
            },
            other => panic!("expected a welcome, got {other:?}"),
        };
        client.drain();
        self.clients[seat] = client;
        welcome
    }
}

/// What a `Welcome` states about a seat's pace.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Welcomed {
    auto_pass: bool,
    full_control: FullControlMode,
    stops: Vec<Stop>,
    yielding: Option<YieldUntil>,
}

impl Welcomed {
    /// The auto-pass flag, the mode and the stops.
    fn settings(&self) -> (bool, FullControlMode, Vec<Stop>) {
        (self.auto_pass, self.full_control, self.stops.clone())
    }
}

fn stops_told(messages: &[ServerMessage]) -> Vec<Vec<Stop>> {
    messages
        .iter()
        .filter_map(|m| match m {
            ServerMessage::Stops { stops } => Some(stops.clone()),
            _ => None,
        })
        .collect()
}

fn modes_told(messages: &[ServerMessage]) -> Vec<FullControlMode> {
    messages
        .iter()
        .filter_map(|m| match m {
            ServerMessage::FullControl { mode } => Some(*mode),
            _ => None,
        })
        .collect()
}

fn yields_told(messages: &[ServerMessage]) -> Vec<Option<YieldUntil>> {
    messages
        .iter()
        .filter_map(|m| match m {
            ServerMessage::Yielding { until } => Some(*until),
            _ => None,
        })
        .collect()
}

/// The kinds of message in the order sent, a run of event batches counted once.
fn order(messages: &[ServerMessage]) -> Vec<&'static str> {
    let mut out: Vec<&'static str> = Vec::new();
    for message in messages {
        let now = match message {
            ServerMessage::Events(_) => "events",
            ServerMessage::Decision { .. } => "decision",
            ServerMessage::Stops { .. } => "stops",
            ServerMessage::FullControl { .. } => "full control",
            ServerMessage::Yielding { .. } => "yielding",
            ServerMessage::Error { .. } => "error",
            ServerMessage::GameOver { .. } => "game over",
            ServerMessage::Snapshot { .. } => "snapshot",
            ServerMessage::Welcome { .. } => "welcome",
            ServerMessage::Chat { .. } => "chat",
            ServerMessage::Forecast { .. } => "forecast",
            ServerMessage::Lobby { .. } => "lobby",
        };
        if now != "events" || out.last().is_none_or(|last| *last != "events") {
            out.push(now);
        }
    }
    out
}

fn is_error(messages: &[ServerMessage], expected: ErrorCode) -> bool {
    matches!(messages, [ServerMessage::Error { code, .. }] if *code == expected)
}

// ---------------------------------------------------------------------------------------------------------------
// The default windows

#[tokio::test]
async fn the_default_windows_ask_a_seat_that_can_act_in_its_own_main_phases_and_a_seat_that_cannot_never()
 {
    // Seat 1 holds an instant that costs nothing; seat 0 holds nothing and draws things it cannot cast.
    let setup = quiet().hand(1, [instant("Zap", 0, &[])]).setup().unwrap();
    let mut table = Table::join(setup, windowed(), 2).await;
    table.start().await;

    let (asks, _) = table.run_to_turn(5).await;
    let at = |turn, step| Ask {
        seat: ME,
        turn,
        step,
    };
    assert_eq!(
        asks,
        [
            at(2, Step::Main1),
            at(2, Step::Main2),
            at(4, Step::Main1),
            at(4, Step::Main2),
        ],
        "seat 1 only in its own main phases, seat 0 never"
    );
}

#[rstest]
#[tokio::test]
async fn an_object_of_another_seat_is_a_window_for_a_seat_that_can_act_and_for_none_that_cannot(
    #[values(true, false)] can_answer: bool,
) {
    let hand = if can_answer {
        vec![instant("My Zap", 0, &[])]
    } else {
        Vec::new()
    };
    let setup = quiet()
        .hand(0, [instant("Foe Zap", 0, &[])])
        .hand(1, hand)
        .setup()
        .unwrap();
    let mut table = Table::join(setup, windowed(), 2).await;
    // Seat 0 is the hand-driven seat: asked everywhere, so it can cast.
    table.send(FOE, mode(FullControlMode::Held)).await;
    table.start().await;

    let asked = table.window().await.unwrap();
    assert_eq!(asked.ask.seat, FOE);
    let DecisionKind::Priority(options) = &asked.decision.kind else {
        panic!("expected priority");
    };
    let spell = options.castable[0];
    table
        .answer(&asked, Answer::Priority(PriorityAction::CastSpell(spell)))
        .await;
    let pay = table.window().await.unwrap();
    table.answer(&pay, Answer::PayMana(PayChoice::Auto)).await;
    let holding = table.window().await.unwrap();
    assert_eq!(holding.ask.seat, FOE, "the caster holds priority again");
    table.pass(&holding).await;

    let window = table.window().await.unwrap();
    if can_answer {
        // Seat 0's object is on the stack and seat 1 can answer it: its window.
        assert_eq!(window.ask.seat, ME);
        assert_eq!((window.ask.turn, window.ask.step), (1, Step::Main1));
    } else {
        // Seat 1 has nothing to answer with, so it was passed for, the spell resolved and seat 0 holds priority.
        assert_eq!(window.ask.seat, FOE);
        assert!(
            table
                .take(ME)
                .iter()
                .all(|m| !matches!(m, ServerMessage::Decision { .. }))
        );
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Stops

#[tokio::test]
async fn a_stop_asks_a_seat_that_cannot_act_and_a_one_turn_stop_goes_with_its_step_over_several_turns()
 {
    let setup = quiet().at(1, 0, Step::Upkeep).setup().unwrap();
    let mut table = Table::join(setup, windowed(), 2).await;
    let (a, b, c) = (
        opponent(Step::Upkeep, false),
        opponent(Step::Draw, true),
        own(Step::End, false),
    );
    for stop in [a, b, c] {
        table.send(ME, set(stop)).await;
    }
    table.take(ME);
    table.start().await;

    let mut asks = Vec::new();
    let mut lists = Vec::new();
    while let Some(asked) = table.window().await {
        if asked.ask.turn > 4 {
            break;
        }
        // Whatever the seat was told since its last answer comes before the decision, after the events.
        lists.extend(stops_told(&asked.told));
        let shape = order(&asked.told);
        assert!(shape.ends_with(&["decision"]), "{shape:?}");
        asks.push(asked.ask);
        table.pass(&asked).await;
    }
    let at = |turn, step| Ask {
        seat: ME,
        turn,
        step,
    };
    assert_eq!(
        asks,
        [
            at(1, Step::Upkeep),
            at(1, Step::Draw),
            at(2, Step::End),
            at(3, Step::Draw),
        ],
        "the lasting stop comes round with every turn of seat 0, the one-turn stops once"
    );
    assert_eq!(
        lists,
        [vec![c, b], vec![b]],
        "told as each went, whole, own side first"
    );
    assert_eq!(table.welcome(ME).await.stops, [b]);
}

#[tokio::test]
async fn the_seat_is_told_a_cleared_stop_after_the_events_and_before_the_next_decision() {
    let setup = quiet().at(1, 0, Step::Upkeep).setup().unwrap();
    let mut table = Table::join(setup, windowed(), 2).await;
    table.send(ME, set(opponent(Step::Upkeep, false))).await;
    table.send(ME, set(opponent(Step::End, true))).await;
    table.take(ME);
    table.start().await;

    let first = table.window().await.unwrap();
    assert_eq!((first.ask.seat, first.ask.step), (ME, Step::Upkeep));
    table.pass(&first).await;
    let second = table.window().await.unwrap();
    assert_eq!((second.ask.seat, second.ask.step), (ME, Step::End));
    assert_eq!(
        order(&second.told),
        ["events", "stops", "decision"],
        "the notice follows the events of the pass that ended the step and comes before the next window"
    );
    assert_eq!(stops_told(&second.told), [vec![opponent(Step::End, true)]]);
}

#[rstest]
#[case::set_in_the_step_it_covers(Step::Main1, Step::Main1, true)]
#[case::set_after_its_step_passed(Step::Main2, Step::Main1, false)]
#[tokio::test]
async fn a_stop_set_while_its_step_runs_covers_that_step_and_one_set_after_it_waits_for_the_next_turn(
    #[case] set_in: Step,
    #[case] stop_at: Step,
    #[case] asked_this_turn: bool,
) {
    let setup = quiet().at(1, 0, set_in).setup().unwrap();
    let mut table = Table::join(setup, windowed(), 2).await;
    // Seat 0 is hand-driven, so the game waits at `set_in` with the step still running.
    table.send(FOE, mode(FullControlMode::Held)).await;
    table.start().await;
    let first = table.window().await.unwrap();
    assert_eq!((first.ask.seat, first.ask.step), (FOE, set_in));

    table.send(ME, set(opponent(stop_at, false))).await;
    table.take(ME);
    table.pass(&first).await;
    let (_, window) = table.run(|ask| ask.seat == ME).await;
    let window = window.expect("seat 1 is asked at the stop");
    let turn = if asked_this_turn { 1 } else { 3 };
    assert_eq!(
        window.ask,
        Ask {
            seat: ME,
            turn,
            step: stop_at
        }
    );
    // The stop goes with the step, told after the events of the pass that ended it.
    table.pass(&window).await;
    assert_eq!(stops_told(&table.take(ME)), [Vec::<Stop>::new()]);
}

#[tokio::test]
async fn a_seat_that_leaves_holds_no_stop_and_the_others_keep_theirs() {
    let setup = Scenario::new(3)
        .at(1, 0, Step::Upkeep)
        .library(0, vec![giant(); 30])
        .library(1, vec![giant(); 30])
        .library(2, vec![giant(); 30])
        .setup()
        .unwrap();
    let mut table = Table::join(setup, windowed(), 3).await;
    for holder in [1, 2] {
        table.send(holder, set(opponent(Step::Upkeep, true))).await;
    }
    table.start().await;

    let first = table.window().await.unwrap();
    assert_eq!((first.ask.seat, first.ask.step), (1, Step::Upkeep));
    table.pass(&first).await;
    let second = table.window().await.unwrap();
    assert_eq!((second.ask.seat, second.ask.step), (2, Step::Upkeep));
    table.send(2, ClientMessage::Concede).await;
    let told = table.take(2);
    assert_eq!(
        stops_told(&told),
        [Vec::<Stop>::new()],
        "told, with the events"
    );
    assert!(
        order(&told).starts_with(&["events", "stops"]),
        "{:?}",
        order(&told)
    );

    // Seat 1 is still asked at seat 0's next upkeep (turn 3: seat 2's turn is gone); seat 2 never is.
    let mut asked = Vec::new();
    while let Some(window) = table.window().await {
        if window.ask.turn > 3 {
            break;
        }
        if window.ask.step == Step::Upkeep {
            asked.push((window.ask.seat, window.ask.turn));
        }
        table.pass(&window).await;
    }
    assert!(asked.iter().all(|(seat, _)| *seat != 2), "{asked:?}");
    assert!(asked.contains(&(1, 3)), "{asked:?}");
}

// ---------------------------------------------------------------------------------------------------------------
// Yields

#[tokio::test]
async fn a_stop_ends_a_yield_at_its_window_with_the_usual_notice_before_the_decision() {
    let setup = quiet().setup().unwrap();
    let mut table = Table::join(setup, windowed(), 2).await;
    table.send(FOE, mode(FullControlMode::Held)).await;
    table.send(ME, set(opponent(Step::End, true))).await;
    table.start().await;
    let first = table.window().await.unwrap();
    assert_eq!(first.ask.seat, FOE);
    table
        .send(
            ME,
            ClientMessage::Yield {
                until: YieldUntil::EndOfTurn,
            },
        )
        .await;
    assert_eq!(yields_told(&table.take(ME)), [Some(YieldUntil::EndOfTurn)]);
    table.pass(&first).await;

    // Seat 0 plays on; the yield passes seat 1 at every window of the turn until the end step, where its stop is.
    let mut window = table.window().await.unwrap();
    while window.ask.seat == FOE {
        table.pass(&window).await;
        window = table.window().await.unwrap();
    }
    assert_eq!(
        (window.ask.seat, window.ask.turn, window.ask.step),
        (ME, 1, Step::End)
    );
    assert_eq!(yields_told(&window.told), [None]);
    assert_eq!(
        order(&window.told),
        ["events", "yielding", "decision"],
        "the yield's end is told after the events and before the window that ended it"
    );
    assert_eq!(table.welcome(ME).await.yielding, None);
}

#[tokio::test]
async fn a_yield_asked_at_a_stop_passes_that_window_and_a_stop_of_a_later_step_ends_it() {
    let setup = quiet().setup().unwrap();
    let mut table = Table::join(setup, windowed(), 2).await;
    table.send(FOE, mode(FullControlMode::Held)).await;
    table.send(ME, set(opponent(Step::Main1, true))).await;
    table.send(ME, set(opponent(Step::End, true))).await;
    table.take(ME);
    table.start().await;
    let (_, first) = table.run(|ask| ask.seat == ME).await;
    let at_main = first.expect("seat 1 is asked at its stop");
    assert_eq!((at_main.ask.turn, at_main.ask.step), (1, Step::Main1));

    // Ending the turn from the stop's own window passes that window instead of meeting the stop again.
    table
        .send(
            ME,
            ClientMessage::Yield {
                until: YieldUntil::EndOfTurn,
            },
        )
        .await;
    let told = table.take(ME);
    assert_eq!(yields_told(&told), [Some(YieldUntil::EndOfTurn)]);
    assert!(
        !told
            .iter()
            .any(|m| matches!(m, ServerMessage::Decision { .. })),
        "the window was passed, not sent again: {told:?}"
    );
    let stale = ClientMessage::Answer {
        decision: at_main.id,
        answer: passive(&at_main.decision),
    };
    table.send(ME, stale).await;
    assert!(is_error(&table.take(ME), ErrorCode::StaleDecision));

    // The stop of a later step still takes the yield back.
    let (_, later) = table.run(|ask| ask.seat == ME).await;
    let at_end = later.expect("seat 1 is asked at its second stop");
    assert_eq!((at_end.ask.turn, at_end.ask.step), (1, Step::End));
    assert_eq!(yields_told(&at_end.told), [None]);
}

// ---------------------------------------------------------------------------------------------------------------
// Full Control

#[tokio::test]
async fn held_asks_at_every_window_of_the_opponents_turn_and_next_window_at_the_next_one_only() {
    for (start, windows_on_seat_0s_turn) in [
        (FullControlMode::Held, 7),
        (FullControlMode::NextWindow, 1),
        (FullControlMode::Off, 0),
    ] {
        let setup = quiet().at(1, 0, Step::Upkeep).setup().unwrap();
        let mut table = Table::join(setup, windowed(), 2).await;
        table.send(ME, mode(start)).await;
        table.start().await;
        let (asks, _) = table.run_to_turn(2).await;
        let asked: Vec<Ask> = asks.into_iter().filter(|ask| ask.seat == ME).collect();
        assert_eq!(asked.len(), windows_on_seat_0s_turn, "{start:?}: {asked:?}");
        if start == FullControlMode::Held {
            let steps: Vec<Step> = asked.iter().map(|ask| ask.step).collect();
            assert_eq!(
                steps,
                [
                    Step::Upkeep,
                    Step::Draw,
                    Step::Main1,
                    Step::BeginCombat,
                    Step::EndCombat,
                    Step::Main2,
                    Step::End
                ],
                "every step where seat 0 passes priority to it"
            );
        }
    }
}

#[tokio::test]
async fn next_window_ends_with_the_seats_own_pass_told_after_the_events_and_before_the_next_decision()
 {
    let setup = quiet().at(1, 0, Step::Upkeep).setup().unwrap();
    let mut table = Table::join(setup, windowed(), 2).await;
    table.send(ME, mode(FullControlMode::NextWindow)).await;
    table.take(ME);
    table.start().await;

    let first = table.window().await.unwrap();
    assert_eq!(
        (first.ask.seat, first.ask.turn, first.ask.step),
        (ME, 1, Step::Upkeep)
    );
    assert!(
        modes_told(&first.told).is_empty(),
        "nothing yet: the seat has not passed"
    );
    table.pass(&first).await;
    // Nothing is a window for either seat any more, so the table passes on for many turns, until the hand of seat 1
    // outgrows its limit: that discard is the next thing it is asked.
    let next = table.window().await.unwrap();
    assert!(
        matches!(next.decision.kind, DecisionKind::Discard(_)),
        "{:?}",
        next.decision
    );
    assert_eq!(modes_told(&next.told), [FullControlMode::Off]);
    assert_eq!(&order(&next.told)[..2], ["events", "full control"]);
    assert_eq!(
        table.welcome(ME).await.settings(),
        (true, FullControlMode::Off, Vec::new())
    );
}

#[tokio::test]
async fn next_window_holds_through_what_the_seat_does_and_ends_at_its_pass() {
    let setup = quiet()
        .hand(1, [instant("My Zap", 0, &[])])
        .at(1, 0, Step::Upkeep)
        .setup()
        .unwrap();
    let mut table = Table::join(setup, windowed(), 2).await;
    table.send(ME, mode(FullControlMode::NextWindow)).await;
    table.start().await;

    let window = table.window().await.unwrap();
    let DecisionKind::Priority(options) = &window.decision.kind else {
        panic!("expected priority");
    };
    let spell = options.castable[0];
    table
        .answer(&window, Answer::Priority(PriorityAction::CastSpell(spell)))
        .await;
    let pay = table.window().await.unwrap();
    table.answer(&pay, Answer::PayMana(PayChoice::Auto)).await;
    let again = table.window().await.unwrap();
    assert_eq!(
        again.ask.seat, ME,
        "seat 1 holds priority again over its spell"
    );
    assert!(modes_told(&again.told).is_empty(), "casting is not passing");
    table.pass(&again).await;
    assert_eq!(modes_told(&table.take(ME)), [FullControlMode::Off]);
}

#[tokio::test(start_paused = true)]
async fn next_window_also_ends_when_the_clock_passes_for_the_seat() {
    let setup = quiet().at(1, 0, Step::Upkeep).setup().unwrap();
    let config = GameConfig {
        pace: CLOCKS,
        ..windowed()
    };
    let mut table = Table::join(setup, config, 2).await;
    table.send(ME, mode(FullControlMode::NextWindow)).await;
    table.start().await;
    let first = table.window().await.unwrap();
    assert_eq!(first.ask.seat, ME);
    silent(&table.take(ME));

    tokio::time::advance(Duration::from_secs(10)).await;
    for _ in 0..50 {
        tokio::task::yield_now().await;
        table.handle.status().await.unwrap();
    }
    let told = table.take(ME);
    assert_eq!(modes_told(&told), [FullControlMode::Off], "{told:?}");
}

#[rstest]
#[case::next_window(FullControlMode::NextWindow)]
#[case::held(FullControlMode::Held)]
#[tokio::test]
async fn a_yield_is_refused_in_full_control_and_nothing_changes(#[case] held: FullControlMode) {
    let setup = quiet().setup().unwrap();
    let mut table = Table::join(setup, windowed(), 2).await;
    table.send(FOE, mode(FullControlMode::Held)).await;
    table.send(ME, mode(held)).await;
    table.start().await;
    table.take(ME);

    for until in [
        YieldUntil::Resolve,
        YieldUntil::EndOfTurn,
        YieldUntil::MyTurn,
    ] {
        table.send(ME, ClientMessage::Yield { until }).await;
        let told = table.take(ME);
        assert!(
            is_error(&told, ErrorCode::BadMessage),
            "{until:?}: {told:?}"
        );
    }
    assert_eq!(table.welcome(ME).await.yielding, None);
    assert_eq!(table.welcome(ME).await.full_control, held);
}

#[rstest]
#[case::held(mode(FullControlMode::Held), FullControlMode::Held)]
#[case::next_window(mode(FullControlMode::NextWindow), FullControlMode::NextWindow)]
#[case::auto_pass_off(ClientMessage::AutoPass { enabled: false }, FullControlMode::Held)]
#[tokio::test]
async fn turning_full_control_on_ends_a_running_yield_with_its_notice_first(
    #[case] turn_on: ClientMessage,
    #[case] told_mode: FullControlMode,
) {
    let setup = quiet().setup().unwrap();
    let mut table = Table::join(setup, windowed(), 2).await;
    table.send(FOE, mode(FullControlMode::Held)).await;
    table.start().await;
    table.take(ME);
    table
        .send(
            ME,
            ClientMessage::Yield {
                until: YieldUntil::MyTurn,
            },
        )
        .await;
    assert_eq!(yields_told(&table.take(ME)), [Some(YieldUntil::MyTurn)]);

    table.send(ME, turn_on).await;
    let told = table.take(ME);
    assert_eq!(order(&told), ["yielding", "full control"]);
    assert_eq!(yields_told(&told), [None]);
    assert_eq!(modes_told(&told), [told_mode]);
    assert_eq!(table.welcome(ME).await.yielding, None);
}

// ---------------------------------------------------------------------------------------------------------------
// Auto-pass is the Held mode

#[rstest]
#[case::off_to_held(FullControlMode::Off, false, Some(FullControlMode::Held), false)]
#[case::held_to_off(FullControlMode::Held, true, Some(FullControlMode::Off), true)]
#[case::on_when_already_off(FullControlMode::Off, true, None, true)]
#[case::off_when_already_held(FullControlMode::Held, false, None, false)]
#[case::on_leaves_next_window_alone(FullControlMode::NextWindow, true, None, true)]
#[case::off_replaces_next_window(
    FullControlMode::NextWindow,
    false,
    Some(FullControlMode::Held),
    false
)]
#[tokio::test]
async fn auto_pass_is_on_in_every_mode_but_held_and_changing_it_changes_the_mode(
    #[case] start: FullControlMode,
    #[case] enabled: bool,
    #[case] told_mode: Option<FullControlMode>,
    #[case] auto_pass_after: bool,
) {
    let setup = quiet().setup().unwrap();
    for started in [false, true] {
        let mut table = Table::join(setup.clone(), windowed(), 2).await;
        table.send(ME, mode(start)).await;
        if started {
            table.send(FOE, mode(FullControlMode::Held)).await;
            table.start().await;
        }
        let (auto_pass, held, _) = table.welcome(ME).await.settings();
        assert_eq!((auto_pass, held), (start != FullControlMode::Held, start));
        table.take(ME);

        table.send(ME, ClientMessage::AutoPass { enabled }).await;
        let told = table.take(ME);
        assert_eq!(
            modes_told(&told),
            told_mode.into_iter().collect::<Vec<_>>(),
            "started {started}"
        );
        let after = told_mode.unwrap_or(start);
        assert_eq!(
            table.welcome(ME).await.settings(),
            (auto_pass_after, after, Vec::new())
        );
    }
}

#[tokio::test]
async fn a_server_that_starts_seats_without_auto_pass_starts_them_in_held() {
    let setup = quiet().setup().unwrap();
    let config = GameConfig {
        pace: Pace::MANUAL,
        ..windowed()
    };
    let mut table = Table::join(setup, config, 2).await;
    for seat in [FOE, ME] {
        assert_eq!(
            table.welcome(seat).await.settings(),
            (false, FullControlMode::Held, Vec::new())
        );
    }
    table.take(ME);
    table
        .send(ME, ClientMessage::AutoPass { enabled: true })
        .await;
    assert_eq!(modes_told(&table.take(ME)), [FullControlMode::Off]);
    assert_eq!(
        table.welcome(ME).await.settings(),
        (true, FullControlMode::Off, Vec::new())
    );
    assert_eq!(table.welcome(FOE).await.full_control, FullControlMode::Held);
}

#[tokio::test]
async fn turning_the_mode_off_at_a_window_the_rules_pass_for_passes_at_once_and_keeps_one_it_asks()
{
    // Seat 1 is in Held, facing its first window (seat 0's main phase), which is no window by the defaults.
    let setup = quiet()
        .hand(1, [instant("My Zap", 0, &[])])
        .setup()
        .unwrap();
    let mut table = Table::join(setup, windowed(), 2).await;
    table.send(FOE, mode(FullControlMode::Held)).await;
    table.send(ME, mode(FullControlMode::Held)).await;
    table.start().await;
    let first = table.window().await.unwrap();
    table.pass(&first).await;
    let facing = table.window().await.unwrap();
    assert_eq!((facing.ask.seat, facing.ask.step), (ME, Step::Main1));

    table.send(ME, mode(FullControlMode::Off)).await;
    let told = table.take(ME);
    assert_eq!(modes_told(&told), [FullControlMode::Off]);
    assert!(
        !told
            .iter()
            .any(|m| matches!(m, ServerMessage::Decision { .. })),
        "passed for at once: {told:?}"
    );
    let stale = ClientMessage::Answer {
        decision: facing.id,
        answer: passive(&facing.decision),
    };
    table.send(ME, stale).await;
    assert!(is_error(&table.take(ME), ErrorCode::StaleDecision));
}

// ---------------------------------------------------------------------------------------------------------------
// A failed save

#[tokio::test]
async fn a_failed_save_keeps_the_stops_and_the_mode_and_the_retry_changes_them() {
    let setup = quiet().at(1, 0, Step::Upkeep).setup().unwrap();
    let mut table = Table::join(setup, windowed(), 2).await;
    let one_turn = opponent(Step::Upkeep, false);
    let lasting = opponent(Step::End, true);
    table.send(ME, set(one_turn)).await;
    table.send(ME, set(lasting)).await;
    table.send(ME, mode(FullControlMode::NextWindow)).await;
    table.start().await;
    let window = table.window().await.unwrap();
    assert_eq!((window.ask.seat, window.ask.step), (ME, Step::Upkeep));
    let events = table.handle.status().await.unwrap().events;

    // The pass that would clear the stop and end the mode cannot be saved.
    table.store.fail(true);
    table.pass(&window).await;
    let told = table.take(ME);
    assert!(is_error(&told, ErrorCode::Internal), "{told:?}");
    assert_eq!(table.handle.status().await.unwrap().events, events);
    assert_eq!(
        table.welcome(ME).await.settings(),
        (true, FullControlMode::NextWindow, vec![one_turn, lasting]),
        "both as they were"
    );

    // Once it can be saved the same decision is answered and both change, told after the events.
    table.store.fail(false);
    table.pass(&window).await;
    let told = table.take(ME);
    assert_eq!(stops_told(&told), [vec![lasting]]);
    assert_eq!(modes_told(&told), [FullControlMode::Off]);
    assert!(
        order(&told).starts_with(&["events", "stops", "full control"]),
        "{:?}",
        order(&told)
    );
}

#[tokio::test]
async fn a_failed_save_keeps_the_mode_a_seat_asked_to_leave_at_a_window_that_would_be_passed() {
    let setup = quiet().setup().unwrap();
    let mut table = Table::join(setup, windowed(), 2).await;
    table.send(FOE, mode(FullControlMode::Held)).await;
    table.send(ME, mode(FullControlMode::Held)).await;
    table.start().await;
    let first = table.window().await.unwrap();
    table.pass(&first).await;
    let facing = table.window().await.unwrap();
    assert_eq!(facing.ask.seat, ME);
    let before = table.handle.status().await.unwrap();

    table.store.fail(true);
    table
        .send(ME, ClientMessage::AutoPass { enabled: true })
        .await;
    let told = table.take(ME);
    assert!(is_error(&told, ErrorCode::Internal), "{told:?}");
    assert_eq!(table.handle.status().await.unwrap(), before);
    assert_eq!(table.welcome(ME).await.full_control, FullControlMode::Held);

    table.store.fail(false);
    table
        .send(ME, ClientMessage::AutoPass { enabled: true })
        .await;
    assert_eq!(modes_told(&table.take(ME)), [FullControlMode::Off]);
    assert!(table.handle.status().await.unwrap().events > before.events);
}

// ---------------------------------------------------------------------------------------------------------------
// The clock

#[rstest]
#[tokio::test(start_paused = true)]
async fn every_delivered_window_of_a_seat_that_is_not_the_active_player_has_the_same_clock(
    #[values(false, true)] can_act: bool,
    #[values("a stop", "full control")] why: &str,
) {
    let hand = if can_act {
        vec![instant("My Zap", 0, &[])]
    } else {
        Vec::new()
    };
    let setup = quiet().hand(1, hand).setup().unwrap();
    let config = GameConfig {
        pace: CLOCKS,
        ..windowed()
    };
    let mut table = Table::join(setup, config, 2).await;
    table.send(FOE, mode(FullControlMode::Held)).await;
    if why == "a stop" {
        table.send(ME, set(opponent(Step::Main1, true))).await;
    } else {
        table.send(ME, mode(FullControlMode::Held)).await;
    }
    table.start().await;
    let first = table.window().await.unwrap();
    assert_eq!(first.ask.seat, FOE);
    table.pass(&first).await;

    let told = {
        let window = table.window().await.unwrap();
        assert_eq!(window.ask.seat, ME);
        window.told
    };
    let clock = told.iter().rev().find_map(|m| match m {
        ServerMessage::Decision { deadline_ms, .. } => Some(*deadline_ms),
        _ => None,
    });
    assert_eq!(clock, Some(Some(10_000)), "can act {can_act}, {why}");
}

// ---------------------------------------------------------------------------------------------------------------
// Bot games against the reference

/// Plays a bot game through a server whose seats start in the mode `auto_pass` names, each bot first choosing its own
/// stops and mode when `chosen`, and checks the log, the hash and the result against the same game on a bare engine
/// under the reference windows.
async fn bots_play_the_reference_game(players: usize, seed: u64, auto_pass: bool, chosen: bool) {
    let pace = Pace {
        auto_pass,
        response: None,
        blocks: None,
        dwell: Dwell::OFF,
    };
    let setup = table_setup(players, seed);
    let store = MemoryStore::new();
    let config = GameConfig { pace, ..windowed() };
    let handle = spawn_game_with(game_id(1), setup.clone(), store.clone(), config).unwrap();
    let mut bots = Vec::new();
    for n in 0..players {
        let mut bot = Bot::join(&handle, n, seed).await;
        if chosen {
            bot.choose_windows(&handle).await;
        }
        bots.push(bot);
    }
    let reports = play_bots(&handle, bots).await;

    let straight = reference_windows(setup, seed, auto_pass, chosen);
    assert_eq!(store.load(game_id(1)).await.unwrap(), straight.events);
    assert_eq!(
        handle.status().await.unwrap().state_hash,
        Some(straight.final_hash)
    );
    let asked: usize = reports.iter().map(|r| r.answers).sum();
    assert_eq!(
        asked,
        straight.answers - straight.auto_passes,
        "the bots were asked exactly where the reference asks"
    );
    for report in &reports {
        assert_eq!(report.result, straight.result);
        assert_eq!(report.errors, [], "{:?}", report.seat);
        if auto_pass && !chosen {
            assert_eq!(
                report.idle_decisions, 0,
                "{:?} was asked for nothing",
                report.seat
            );
        }
    }
}

#[rstest]
#[tokio::test]
async fn bot_games_equal_the_reference_under_the_default_windows(
    #[values(2, 3, 4)] players: usize,
    #[values(1, 2, 3, 4)] seed: u64,
) {
    bots_play_the_reference_game(players, seed, true, false).await;
}

#[rstest]
#[tokio::test]
async fn bot_games_equal_the_reference_when_every_bot_chooses_its_stops_and_full_control(
    #[values(2, 3, 4)] players: usize,
    #[values(1, 2, 3, 4, 5, 6)] seed: u64,
    #[values(true, false)] auto_pass: bool,
) {
    bots_play_the_reference_game(players, seed, auto_pass, true).await;
}

#[rstest]
#[tokio::test]
async fn bot_games_equal_the_reference_with_every_seat_in_full_control(
    #[values(2, 4)] players: usize,
    #[values(1, 2)] seed: u64,
) {
    bots_play_the_reference_game(players, seed, false, false).await;
}
