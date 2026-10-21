//! Goal: a bot client that plays through the handle API the way a real client would, and checks the stream it receives.

use mtg_core::ids::PlayerId;
use mtg_engine::decision::PriorityAction;
use mtg_engine::state::GameResult;
use mtg_engine::{Answer, Decision, DecisionKind};
use mtg_protocol::{ClientMessage, DecisionId, ErrorCode, ServerMessage, YieldUntil};
use mtg_server::GameHandle;
use mtg_testkit::RandomBot;
use mtg_testkit::SeatWindows;
use tokio::sync::mpsc::Receiver;

use super::timed;

/// What handling one server message led to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Progress {
    /// Nothing special.
    Continue,
    /// The game is over.
    Over(GameResult),
}

/// A seeded random player.
///
/// It asserts what every well-behaved client may rely on: decisions are addressed to its own seat,
/// and event batches continue exactly where the previous batch or snapshot ended.
pub struct Bot {
    pub seat: PlayerId,
    pub inbox: Receiver<ServerMessage>,
    brain: RandomBot,
    /// An answer decided but not sent (see `drive`), repeated when the same decision shows up again.
    held: Option<(Decision, Answer)>,
    /// Concede instead of answering the n-th decision (counting from 1).
    pub concede_at: Option<usize>,
    /// Ask for this yield each time the bot would pass priority, instead of passing.
    pub yield_on_pass: Option<YieldUntil>,
    /// The yield the server says the seat holds: from `Welcome` and every `Yielding`.
    pub yielding: Option<YieldUntil>,
    /// `Yield` messages sent.
    pub yields: usize,
    /// Objects on the stack as of the last batch or snapshot.
    stack: u32,
    pub next_seq: u64,
    pub decisions: usize,
    /// Priority decisions it was sent that it could do nothing with (a server that passes for idle seats never sends one).
    pub idle_decisions: usize,
    /// Decisions it was sent on a clock.
    pub clocked: usize,
    pub answers: usize,
    pub snapshots: usize,
    pub errors: Vec<(ErrorCode, String)>,
}

/// What a bot saw by the time the game ended.
#[derive(Debug)]
pub struct BotReport {
    pub seat: PlayerId,
    pub result: GameResult,
    pub yields: usize,
    pub decisions: usize,
    pub idle_decisions: usize,
    pub clocked: usize,
    pub answers: usize,
    pub events_seen: u64,
    pub snapshots: usize,
    pub errors: Vec<(ErrorCode, String)>,
}

impl Bot {
    /// Connects `seat`; the bot plays like `RandomBot::new(seed, seat)`, as in the reference games.
    pub async fn join(handle: &GameHandle, seat: usize, seed: u64) -> Bot {
        let inbox = handle.join(PlayerId::from_index(seat)).await.unwrap();
        Bot {
            seat: PlayerId::from_index(seat),
            inbox,
            brain: RandomBot::new(seed, seat as u64),
            held: None,
            concede_at: None,
            yield_on_pass: None,
            yielding: None,
            yields: 0,
            stack: 0,
            next_seq: 0,
            decisions: 0,
            idle_decisions: 0,
            clocked: 0,
            answers: 0,
            snapshots: 0,
            errors: Vec::new(),
        }
    }

    /// Chooses the stops and Full Control mode this bot plays with, tells the server (as a seat does in the lobby) and
    /// returns the choice. Call it before anything else, as the reference game does.
    pub async fn choose_windows(&mut self, handle: &GameHandle) -> SeatWindows {
        let chosen = self.brain.windows();
        super::send_windows(handle, self.seat, &chosen).await;
        chosen
    }

    /// Connects the seat again (to a new actor, or after dropping the old queue).
    pub async fn rejoin(&mut self, handle: &GameHandle) {
        self.inbox = handle.join(self.seat).await.unwrap();
    }

    pub fn try_next(&mut self) -> Option<ServerMessage> {
        self.inbox.try_recv().ok()
    }

    /// Reacts to one server message; with `allow` false it decides but holds its answer back.
    pub async fn handle(
        &mut self,
        handle: &GameHandle,
        message: ServerMessage,
        allow: bool,
    ) -> Progress {
        match message {
            ServerMessage::Welcome {
                player, yielding, ..
            } => {
                assert_eq!(player, self.seat);
                self.yielding = yielding;
            }
            ServerMessage::Yielding { until } => self.yielding = until,
            ServerMessage::Snapshot { seq, state } => {
                assert_eq!(state.table.viewer, self.seat);
                self.next_seq = seq;
                self.stack = state.table.stack.count;
                self.snapshots += 1;
            }
            ServerMessage::Events(batch) => {
                assert_eq!(
                    batch.from_seq, self.next_seq,
                    "{:?} saw a gap in the event stream",
                    self.seat
                );
                self.next_seq += batch.events.len() as u64;
                self.stack = batch.table.stack.count;
            }
            ServerMessage::Decision {
                id,
                decision,
                deadline_ms,
            } => {
                assert_eq!(
                    decision.player, self.seat,
                    "a decision reached the wrong seat"
                );
                assert!(
                    self.yielding.is_none() || !matches!(decision.kind, DecisionKind::Priority(_)),
                    "{:?} was sent a priority decision while it yields ({:?})",
                    self.seat,
                    self.yielding
                );
                self.decisions += 1;
                self.idle_decisions +=
                    usize::from(matches!(&decision.kind, DecisionKind::Priority(o) if !o.can_act));
                self.clocked += usize::from(deadline_ms.is_some());
                self.respond(handle, id, decision, allow).await;
            }
            ServerMessage::Error { code, message } => self.errors.push((code, message)),
            // The roster of a lobby is for people; a bot only waits for its first decision.
            ServerMessage::Chat { .. }
            | ServerMessage::Lobby { .. }
            | ServerMessage::Stops { .. }
            | ServerMessage::FullControl { .. } => {}
            ServerMessage::GameOver { result } => return Progress::Over(result),
            ServerMessage::Forecast { .. } => {
                panic!("a bot never previews, so it is sent no forecast")
            }
        }
        Progress::Continue
    }

    async fn respond(
        &mut self,
        handle: &GameHandle,
        id: DecisionId,
        decision: Decision,
        allow: bool,
    ) {
        if self.concede_at == Some(self.decisions) {
            handle
                .send(self.seat, ClientMessage::Concede)
                .await
                .unwrap();
            return;
        }
        let answer = match self.held.take() {
            Some((held, answer)) => {
                assert_eq!(held, decision, "the game moved on while an answer was held");
                answer
            }
            None => self.brain.choose(&decision),
        };
        if allow {
            // Resolving an empty stack would end at once, so a client asks for it only over a spell.
            if let (Some(until), Answer::Priority(PriorityAction::Pass)) =
                (self.yield_on_pass, &answer)
                && (until != YieldUntil::Resolve || self.stack > 0)
            {
                self.yields += 1;
                let message = ClientMessage::Yield { until };
                handle.send(self.seat, message).await.unwrap();
                return;
            }
            self.answers += 1;
            let message = ClientMessage::Answer {
                decision: id,
                answer,
            };
            handle.send(self.seat, message).await.unwrap();
        } else {
            self.held = Some((decision, answer));
        }
    }

    /// Plays until the game is over; the queue closing first is a failure.
    pub async fn play(mut self, handle: GameHandle) -> BotReport {
        loop {
            let message = timed(self.inbox.recv()).await.unwrap_or_else(|| {
                panic!(
                    "{:?}: the server closed the queue before GameOver",
                    self.seat
                )
            });
            if let Progress::Over(result) = self.handle(&handle, message, true).await {
                return BotReport {
                    seat: self.seat,
                    result,
                    yields: self.yields,
                    decisions: self.decisions,
                    idle_decisions: self.idle_decisions,
                    clocked: self.clocked,
                    answers: self.answers,
                    events_seen: self.next_seq,
                    snapshots: self.snapshots,
                    errors: self.errors,
                };
            }
        }
    }
}

/// Seats `players` bots, readies them all and plays the game out with one task per bot.
pub async fn play_table(handle: &GameHandle, players: usize, seed: u64) -> Vec<BotReport> {
    let mut bots = Vec::new();
    for seat in 0..players {
        bots.push(Bot::join(handle, seat, seed).await);
    }
    play_bots(handle, bots).await
}

/// Readies the seated bots and plays the game out with one task per bot.
pub async fn play_bots(handle: &GameHandle, bots: Vec<Bot>) -> Vec<BotReport> {
    for bot in &bots {
        handle.send(bot.seat, ClientMessage::Ready).await.unwrap();
    }
    let tasks: Vec<_> = bots
        .into_iter()
        .map(|bot| tokio::spawn(bot.play(handle.clone())))
        .collect();
    let mut reports = Vec::new();
    for task in tasks {
        reports.push(task.await.unwrap());
    }
    reports
}

/// Runs the bots one message at a time on this task until nothing is left to do.
///
/// After `limit` answers have been sent the bots keep deciding but hold their answers back, so the
/// game stops at an exactly known point. Returns the result if the game ended.
pub async fn drive(bots: &mut [Bot], handle: &GameHandle, limit: usize) -> Option<GameResult> {
    let mut over = None;
    let mut sent: usize = bots.iter().map(|b| b.answers).sum();
    loop {
        // The actor handles commands in order, so once this returns it has queued every reply to
        // everything sent so far; a pass that then finds nothing to read means the table is quiet.
        handle.status().await.unwrap();
        let mut busy = false;
        for bot in bots.iter_mut() {
            while let Some(message) = bot.try_next() {
                busy = true;
                let before = bot.answers;
                if let Progress::Over(result) = bot.handle(handle, message, sent < limit).await {
                    over = Some(result);
                }
                sent += bot.answers - before;
            }
        }
        if !busy {
            return over;
        }
    }
}
