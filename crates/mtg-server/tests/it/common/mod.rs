//! Goal: shared helpers for the integration tests: bot and manual clients that talk to a game only through its handle, a failing store double and reference games.
#![allow(dead_code, unused_imports, clippy::unwrap_used, clippy::expect_used)]

pub mod bot;
pub mod cdn;
pub mod logs;
pub mod store;
pub mod tape;

use std::future::Future;
use std::time::Duration;

use mtg_core::ids::PlayerId;
use mtg_engine::state::GameResult;
use mtg_engine::{Event, GameSetup};
use mtg_protocol::{ClientMessage, DecisionId, FullControlMode, ServerMessage, StopSide};
use mtg_server::{Dwell, GameConfig, GameHandle, GameId, Pace};
use mtg_testkit::windows::{Mode, SeatWindows, Side};
use mtg_testkit::{PlayOptions, fuzz_setup, play_game, play_game_with};
use tokio::sync::mpsc::Receiver;
use uuid::Uuid;

pub use bot::{Bot, BotReport, Progress, drive, play_bots, play_table};
pub use logs::capture_logs;
pub use store::FlakyStore;
pub use tape::{Tape, Taped, kind, ms};

/// A game id derived from a number, so tests stay readable.
pub fn game_id(n: u128) -> GameId {
    GameId(Uuid::from_u128(n))
}

/// Seat `n`.
pub fn seat(n: usize) -> PlayerId {
    PlayerId::from_index(n)
}

/// Every wait in a test is bounded, so a bug shows up as a failure instead of a hang.
pub async fn timed<T>(future: impl Future<Output = T>) -> T {
    tokio::time::timeout(Duration::from_secs(120), future)
        .await
        .expect("timed out waiting for the game")
}

/// Queues roomy enough that bulk bot games never lose a client by accident, and no auto-pass or clocks, so a game
/// plays out exactly as the same bots do on a bare engine.
pub fn roomy() -> GameConfig {
    GameConfig {
        seat_queue: 8192,
        command_queue: 64,
        pace: Pace::MANUAL,
    }
}

/// Roomy queues, the default windows (every seat starts in Full Control `Off`) and no clocks, so only the windows,
/// stops, modes and yields under test decide when a seat is asked.
pub fn windowed() -> GameConfig {
    GameConfig {
        pace: Pace {
            auto_pass: true,
            response: None,
            blocks: None,
            think: None,
            active: None,
            extension: None,
            grace: std::time::Duration::ZERO,
            dwell: Dwell::OFF,
        },
        ..roomy()
    }
}

/// The setup every table test uses: random 60-card decks derived from `seed`.
pub fn table_setup(players: usize, seed: u64) -> GameSetup {
    fuzz_setup(players, seed)
}

/// The same game played straight on an engine with the same bots, as the yardstick for actor games.
pub struct Reference {
    pub events: Vec<Event>,
    pub final_hash: u64,
    pub result: GameResult,
    pub answers: usize,
    /// Passes the harness made for holders with nothing to do (0 unless the reference passes for them).
    pub auto_passes: usize,
}

pub fn reference(players: usize, seed: u64) -> Reference {
    let outcome = play_game(fuzz_setup(players, seed), seed).unwrap_or_else(|f| panic!("{f}"));
    Reference {
        events: outcome.events,
        final_hash: outcome.final_hash,
        result: outcome.result,
        answers: outcome.answers,
        auto_passes: outcome.auto_passes,
    }
}

/// The same game where the table passes for a holder with nothing to do, as a server with auto-pass on does.
pub fn reference_auto_pass(setup: GameSetup, seed: u64) -> Reference {
    let options = PlayOptions {
        auto_pass: true,
        ..PlayOptions::default()
    };
    let outcome = play_game_with(setup, seed, options).unwrap_or_else(|f| panic!("{f}"));
    Reference {
        events: outcome.events,
        final_hash: outcome.final_hash,
        result: outcome.result,
        answers: outcome.answers,
        auto_passes: outcome.auto_passes,
    }
}

/// The same game played on a bare engine under the reference windows ([`mtg_testkit::Windows`]): every seat starts in
/// the mode the server default names (`auto_pass`), and with `bot_windows` each bot first chooses its own stops and
/// mode, which [`Bot::choose_windows`] sends to the server.
pub fn reference_windows(
    setup: GameSetup,
    seed: u64,
    auto_pass: bool,
    bot_windows: bool,
) -> Reference {
    let options = PlayOptions {
        auto_pass,
        bot_windows,
        ..PlayOptions::default()
    };
    let outcome = play_game_with(setup, seed, options).unwrap_or_else(|f| panic!("{f}"));
    Reference {
        events: outcome.events,
        final_hash: outcome.final_hash,
        result: outcome.result,
        answers: outcome.answers,
        auto_passes: outcome.auto_passes,
    }
}

/// Sends the stops and Full Control mode `windows` names as `seat`, the way a client does before the start.
pub async fn send_windows(handle: &GameHandle, seat: PlayerId, windows: &SeatWindows) {
    for stop in &windows.stops {
        let message = ClientMessage::SetStop {
            side: match stop.side {
                Side::Own => StopSide::Own,
                Side::Opponent => StopSide::Opponent,
            },
            step: stop.step,
            on: true,
            keep: stop.keep,
        };
        handle.send(seat, message).await.unwrap();
    }
    let mode = match windows.mode {
        Mode::Off => FullControlMode::Off,
        Mode::NextWindow => FullControlMode::NextWindow,
        Mode::Held => FullControlMode::Held,
    };
    handle
        .send(seat, ClientMessage::FullControl { mode })
        .await
        .unwrap();
}

/// A hand-driven client: it reads what the server sends and sends whatever the test wants.
pub struct Client {
    pub seat: PlayerId,
    pub inbox: Receiver<ServerMessage>,
}

impl Client {
    pub async fn join(handle: &GameHandle, seat: usize) -> Client {
        let seat = PlayerId::from_index(seat);
        let inbox = handle.join(seat).await.unwrap();
        Client { seat, inbox }
    }

    /// Everything queued for this client right now.
    pub fn drain(&mut self) -> Vec<ServerMessage> {
        let mut messages = Vec::new();
        while let Ok(message) = self.inbox.try_recv() {
            messages.push(message);
        }
        messages
    }

    /// Waits for the next message.
    pub async fn next(&mut self) -> ServerMessage {
        timed(self.inbox.recv())
            .await
            .expect("the server closed this client's queue")
    }

    /// Waits for the first message `pick` accepts, skipping the others.
    pub async fn until<T>(&mut self, pick: impl Fn(&ServerMessage) -> Option<T>) -> T {
        loop {
            let message = self.next().await;
            if let Some(found) = pick(&message) {
                return found;
            }
        }
    }

    /// Waits for a decision and returns its id and content.
    pub async fn decision(&mut self) -> (DecisionId, mtg_engine::Decision) {
        self.until(|m| match m {
            ServerMessage::Decision { id, decision, .. } => Some((*id, decision.clone())),
            _ => None,
        })
        .await
    }

    pub async fn say(&self, handle: &GameHandle, message: ClientMessage) {
        handle.send(self.seat, message).await.unwrap();
    }
}

/// Joins `seats` clients, readies them all and returns them with the seat that moves first.
pub async fn start_table(handle: &GameHandle, seats: usize) -> (Vec<Client>, usize) {
    let mut clients = Vec::new();
    for n in 0..seats {
        clients.push(Client::join(handle, n).await);
    }
    for client in &clients {
        client.say(handle, ClientMessage::Ready).await;
    }
    let first = handle
        .status()
        .await
        .unwrap()
        .waiting_on
        .expect("a started game waits for someone");
    (clients, first.index())
}

/// Sends `message` as seat `from`, waits until the actor has dealt with it, and returns what each client received.
pub async fn poke(
    handle: &GameHandle,
    clients: &mut [Client],
    from: usize,
    message: ClientMessage,
) -> Vec<Vec<ServerMessage>> {
    for client in clients.iter_mut() {
        client.drain();
    }
    handle.send(seat(from), message).await.unwrap();
    // The actor handles commands in order, so after this everything it will send for `message` is queued.
    handle.status().await.unwrap();
    clients.iter_mut().map(Client::drain).collect()
}

/// Asserts that a client received nothing.
pub fn silent(messages: &[ServerMessage]) {
    assert_eq!(messages.len(), 0, "unexpected messages: {messages:?}");
}

/// The roster a lobby whose seats are called "Seat 1", "Seat 2"... sends, from each seat's (connected, ready) flags.
pub fn roster(flags: &[(bool, bool)]) -> ServerMessage {
    let seats = flags
        .iter()
        .enumerate()
        .map(|(index, (connected, ready))| mtg_protocol::LobbySeat {
            seat: seat(index),
            name: format!("Seat {}", index + 1),
            connected: *connected,
            ready: *ready,
            deck: None,
        })
        .collect();
    let needed = flags.iter().filter(|(_, ready)| !ready).count();
    ServerMessage::Lobby {
        seats,
        needed: u32::try_from(needed).unwrap(),
    }
}

/// `messages` without the lobby rosters, for tests about something else that happens before the start.
pub fn without_rosters(messages: Vec<ServerMessage>) -> Vec<ServerMessage> {
    messages
        .into_iter()
        .filter(|message| !matches!(message, ServerMessage::Lobby { .. }))
        .collect()
}

/// A harmless legal answer to any decision: keep, pass, attack with nothing, block nothing.
pub fn passive(decision: &mtg_engine::Decision) -> mtg_engine::Answer {
    use mtg_engine::Answer;
    use mtg_engine::decision::{DecisionKind, MulliganChoice, PayChoice, PriorityAction};
    match &decision.kind {
        DecisionKind::Mulligan(_) => Answer::Mulligan(MulliganChoice::Keep),
        DecisionKind::BottomCards(o) => {
            Answer::BottomCards(o.hand.iter().copied().take(o.count).collect())
        }
        DecisionKind::Priority(_) => Answer::Priority(PriorityAction::Pass),
        DecisionKind::PayMana(_) => Answer::PayMana(PayChoice::Cancel),
        DecisionKind::DeclareAttackers(_) => Answer::DeclareAttackers(Vec::new()),
        DecisionKind::DeclareBlockers(_) => Answer::DeclareBlockers(Vec::new()),
        DecisionKind::AssignDamage(o) => Answer::AssignDamage(
            o.blockers
                .iter()
                .enumerate()
                .map(|(i, b)| (*b, if i == 0 { o.power } else { 0 }))
                .collect(),
        ),
        DecisionKind::Discard(o) => Answer::Discard(o.hand.iter().copied().take(o.count).collect()),
        // These games are vanilla, so no spell or ability ever asks about its effects.
        other => panic!("no passive answer to a {} decision", other.name()),
    }
}

/// Answers passively, one decision at a time, until a priority decision comes up; returns its seat, id and content.
pub async fn passive_until_priority(
    handle: &GameHandle,
    clients: &mut [Client],
) -> (usize, DecisionId, mtg_engine::Decision) {
    for _ in 0..200 {
        let waiting = handle.status().await.unwrap().waiting_on.unwrap().index();
        let (id, decision) = clients[waiting].decision().await;
        if matches!(decision.kind, mtg_engine::DecisionKind::Priority(_)) {
            return (waiting, id, decision);
        }
        let answer = passive(&decision);
        clients[waiting]
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
    panic!("no priority decision came up");
}
