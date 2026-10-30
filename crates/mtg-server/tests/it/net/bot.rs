//! Goal: a seeded random player that plays over a real WebSocket the way the handle-based bot does, asserting the same stream invariants (right seat, gapless events).

use std::net::SocketAddr;

use mtg_core::ids::PlayerId;
use mtg_engine::state::GameResult;
use mtg_engine::{Answer, Decision, DecisionKind};
use mtg_protocol::{ClientMessage, DecisionId, ErrorCode, ServerMessage};
use mtg_testkit::RandomBot;

use super::ws::{Frame, WsClient, connect, open};
use crate::common::bot::BotReport;

/// How a call to [`WsBot::play`] ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stop {
    /// The game is over.
    Over(GameResult),
    /// The bot reached the decision it was told to pause at and holds its answer.
    Paused,
}

pub struct WsBot {
    pub seat: usize,
    pub client: WsClient,
    brain: RandomBot,
    /// An answer decided but not sent, repeated when the same decision comes back after a reconnect.
    held: Option<(Decision, Answer)>,
    /// Stop (without answering) on the n-th decision seen, counting from 1.
    pub pause_at: Option<usize>,
    pub next_seq: u64,
    pub decisions: usize,
    pub idle_decisions: usize,
    pub clocked: usize,
    pub answers: usize,
    pub snapshots: usize,
    pub errors: Vec<(ErrorCode, String)>,
}

impl WsBot {
    /// Opens a socket with the seat's token, sends `Join` and waits for `Welcome`.
    pub async fn connect(
        addr: SocketAddr,
        game: &str,
        token: &str,
        seat: usize,
        seed: u64,
    ) -> WsBot {
        let mut client = connect(addr, game, token).await;
        let player = client.welcome(game).await;
        assert_eq!(player.index(), seat, "the token opened another seat");
        WsBot::new(client, seat, seed)
    }

    /// Like [`WsBot::connect`] for a signed-in user, who gets in with the session cookie
    /// (`mtg_session=<token>`) instead of a seat token.
    pub async fn connect_as_user(
        addr: SocketAddr,
        game: &str,
        cookie: &str,
        seat: usize,
        seed: u64,
    ) -> WsBot {
        let mut client = open(addr, &format!("/ws/{game}"), &[("Cookie", cookie)]).await;
        let player = client.welcome(game).await;
        assert_eq!(player.index(), seat, "the session opened another seat");
        WsBot::new(client, seat, seed)
    }

    fn new(client: WsClient, seat: usize, seed: u64) -> WsBot {
        WsBot {
            seat,
            client,
            brain: RandomBot::new(seed, seat as u64),
            held: None,
            pause_at: None,
            next_seq: 0,
            decisions: 0,
            idle_decisions: 0,
            clocked: 0,
            answers: 0,
            snapshots: 0,
            errors: Vec::new(),
        }
    }

    /// Opens a fresh socket and joins on it, returning the old one; the server answers with Welcome,
    /// a snapshot and the pending decision.
    pub async fn reconnect(&mut self, addr: SocketAddr, game: &str, token: &str) -> WsClient {
        let mut fresh = connect(addr, game, token).await;
        fresh.join(game).await;
        std::mem::replace(&mut self.client, fresh)
    }

    /// Like [`WsBot::reconnect`], through the session cookie alone.
    pub async fn reconnect_as_user(
        &mut self,
        addr: SocketAddr,
        game: &str,
        cookie: &str,
    ) -> WsClient {
        let mut fresh = open(addr, &format!("/ws/{game}"), &[("Cookie", cookie)]).await;
        fresh.join(game).await;
        std::mem::replace(&mut self.client, fresh)
    }

    pub async fn ready(&mut self) {
        self.client.send(&ClientMessage::Ready).await;
    }

    /// Plays until the game is over or the pause point is reached.
    pub async fn play(&mut self) -> Stop {
        loop {
            match self.client.next().await {
                Frame::Message(message) => {
                    if let Some(stop) = self.handle(message).await {
                        return stop;
                    }
                }
                other => panic!("seat {}: {other:?} before GameOver", self.seat),
            }
        }
    }

    /// Plays to the end, hangs up politely and reports what it saw.
    pub async fn finish(self) -> BotReport {
        self.finish_recorded().await.0
    }

    /// Like [`WsBot::finish`], also returning every text frame the final socket received.
    pub async fn finish_recorded(mut self) -> (BotReport, Vec<String>) {
        let Stop::Over(result) = self.play().await else {
            panic!("seat {} paused with no way to resume", self.seat);
        };
        self.client.close().await;
        let report = BotReport {
            seat: PlayerId::from_index(self.seat),
            result,
            yields: 0,
            decisions: self.decisions,
            idle_decisions: self.idle_decisions,
            clocked: self.clocked,
            answers: self.answers,
            events_seen: self.next_seq,
            snapshots: self.snapshots,
            errors: self.errors,
        };
        (report, self.client.texts)
    }

    /// Handles a message the test read itself, as `play` would.
    pub async fn feed(&mut self, message: ServerMessage) -> Option<Stop> {
        self.handle(message).await
    }

    async fn handle(&mut self, message: ServerMessage) -> Option<Stop> {
        let me = PlayerId::from_index(self.seat);
        match message {
            ServerMessage::Welcome { player, .. } => assert_eq!(player, me),
            ServerMessage::Snapshot { seq, state, .. } => {
                assert_eq!(state.table.viewer, me, "a snapshot for another seat");
                self.next_seq = seq;
                self.snapshots += 1;
            }
            ServerMessage::Events(batch) => {
                assert_eq!(
                    batch.from_seq, self.next_seq,
                    "seat {} saw a gap in the event stream",
                    self.seat
                );
                self.next_seq += batch.events.len() as u64;
            }
            ServerMessage::Decision {
                id,
                decision,
                deadline_ms,
                ..
            } => {
                assert_eq!(decision.player, me, "a decision for another seat");
                self.decisions += 1;
                self.idle_decisions +=
                    usize::from(matches!(&decision.kind, DecisionKind::Priority(o) if !o.can_act));
                self.clocked += usize::from(deadline_ms.is_some());
                return self.respond(id, decision).await;
            }
            ServerMessage::Error { code, message } => self.errors.push((code, message)),
            // The roster of a lobby is for people; a bot only waits for its first decision.
            ServerMessage::Chat { .. }
            | ServerMessage::Lobby { .. }
            | ServerMessage::Yielding { .. }
            | ServerMessage::Stops { .. }
            | ServerMessage::FullControl { .. }
            | ServerMessage::Bank { .. } => {}
            ServerMessage::GameOver { result } => return Some(Stop::Over(result)),
            ServerMessage::Forecast { .. } => {
                panic!("a bot never previews, so it is sent no forecast")
            }
        }
        None
    }

    async fn respond(&mut self, id: DecisionId, decision: Decision) -> Option<Stop> {
        let answer = match self.held.take() {
            Some((held, answer)) => {
                assert_eq!(held, decision, "the game moved on while an answer was held");
                answer
            }
            None => self.brain.choose(&decision),
        };
        if self.pause_at == Some(self.decisions) {
            self.pause_at = None;
            self.held = Some((decision, answer));
            return Some(Stop::Paused);
        }
        self.answers += 1;
        let message = ClientMessage::Answer {
            decision: id,
            answer,
        };
        self.client.send(&message).await;
        None
    }
}
