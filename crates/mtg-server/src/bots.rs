//! Bot seats: a server-side driver that plays a seat of a game the way a client would, through its `GameHandle`.

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use mtg_core::ids::PlayerId;
use mtg_engine::Decision;
use mtg_engine::Engine;
use mtg_engine::decision::{Answer, DecisionKind, PriorityAction};
use mtg_protocol::{ClientMessage, DecisionId, ErrorCode, ServerMessage};
use mtg_testkit::brain::{self, Level, SeatBrain};
use serde::{Deserialize, Serialize};
use tokio::sync::{Semaphore, watch};
use tokio::task::JoinHandle;
use tokio::time::{Instant, sleep, sleep_until};

use crate::handle::GameHandle;
use crate::registry::Removal;

/// How long a bot waits before it answers, so the people at the table can follow its plays.
pub const BOT_DELAY: Duration = Duration::from_millis(800);

/// How many answers a bot gives in one turn before it only passes: a loop of free plays cannot hold a turn.
pub const TURN_ANSWERS: u32 = 120;

/// How long a bot waits before it joins its seat again after its connection was dropped.
const REJOIN_AFTER: Duration = Duration::from_secs(1);

/// The least share of its budget a searching bot thinks with, however busy the server is.
const LEAST_SHARE: f64 = 0.2;

/// The share of its budget a searching bot thinks with while `queued` other bots wait for a thinking slot: all of it
/// on a quiet server, less the longer the queue, never under [`LEAST_SHARE`].
pub fn share_for(queued: usize) -> f64 {
    let queued = f64::from(u32::try_from(queued).unwrap_or(u32::MAX));
    (1.0 / (1.0 + queued)).max(LEAST_SHARE)
}

/// The server's slots for thinking bots: at most `capacity` bots search at once, each on a blocking thread, and a
/// bot that finds others waiting thinks with a smaller share of its budget ([`share_for`]). Cheap to clone.
#[derive(Clone, Debug)]
pub struct Thinkers {
    permits: Arc<Semaphore>,
    queued: Arc<AtomicUsize>,
}

impl Thinkers {
    /// Slots for `capacity` bots thinking at once (at least one).
    pub fn new(capacity: usize) -> Thinkers {
        Thinkers {
            permits: Arc::new(Semaphore::new(capacity.max(1))),
            queued: Arc::new(AtomicUsize::new(0)),
        }
    }

    /// The slots a server of this machine gives: one fewer than its cores, at least one.
    pub fn for_machine() -> usize {
        std::thread::available_parallelism()
            .map_or(1, |cores| cores.get().saturating_sub(1))
            .max(1)
    }

    /// Runs `work` on a blocking thread once a slot is free, handing it the share of its budget to think with;
    /// `None` if the thread failed.
    pub async fn think<T: Send + 'static>(
        &self,
        work: impl FnOnce(f64) -> T + Send + 'static,
    ) -> Option<T> {
        self.queued.fetch_add(1, Ordering::SeqCst);
        let permit = Arc::clone(&self.permits).acquire_owned().await;
        self.queued.fetch_sub(1, Ordering::SeqCst);
        let permit = permit.ok()?;
        let share = share_for(self.queued.load(Ordering::SeqCst));
        tokio::task::spawn_blocking(move || {
            let _permit = permit;
            work(share)
        })
        .await
        .ok()
    }
}

/// How well a server bot plays: the `level` of a bot seat on the create form.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BotLevel {
    /// Random legal answers, a land whenever it can play one.
    Easy,
    /// Heuristics with a shallow search of engine copies (`mtg_testkit::brain`).
    #[default]
    Normal,
    /// Heuristics with a deeper search that looks through the next turn's combat.
    Hard,
    /// Monte Carlo search over sampled hidden cards for casts, attacks, blocks and targets.
    #[serde(rename = "very_hard")]
    VeryHard,
}

impl BotLevel {
    /// The brain level that plays it.
    const fn brain(self) -> Level {
        match self {
            BotLevel::Easy => Level::Easy,
            BotLevel::Normal => Level::Normal,
            BotLevel::Hard => Level::Hard,
            BotLevel::VeryHard => Level::VeryHard,
        }
    }
}

/// A seat a server bot plays, and how well. Stored in the recipe as `{seat, level}`; a bare seat number, as older
/// recipes hold, reads as an Easy bot (how bots played then).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct BotSeat {
    /// The seat.
    pub seat: PlayerId,
    /// How well its bot plays.
    pub level: BotLevel,
}

/// A bot seat as stored: a bare seat number or a seat with its level.
#[derive(Deserialize)]
#[serde(untagged)]
enum StoredBot {
    Seat(PlayerId),
    Leveled { seat: PlayerId, level: BotLevel },
}

impl<'de> Deserialize<'de> for BotSeat {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Ok(match StoredBot::deserialize(deserializer)? {
            StoredBot::Seat(seat) => BotSeat {
                seat,
                level: BotLevel::Easy,
            },
            StoredBot::Leveled { seat, level } => BotSeat { seat, level },
        })
    }
}

/// What the bot of one seat decides with: the [`SeatBrain`] of its level, and a pass once it has answered
/// [`TURN_ANSWERS`] times in a turn.
pub struct BotBrain {
    /// The brain; out of its place only while it thinks on a blocking thread.
    brain: Option<Box<dyn SeatBrain>>,
    level: BotLevel,
    turn: Option<u32>,
    answers: u32,
}

impl std::fmt::Debug for BotBrain {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BotBrain")
            .field("level", &self.level)
            .field("turn", &self.turn)
            .field("answers", &self.answers)
            .finish_non_exhaustive()
    }
}

impl BotBrain {
    /// The brain of `seat` at `level` in a game shuffled with `seed`: an Easy bot's random choices replay with the seed;
    /// a searching bot is also capped by its clock.
    pub fn new(seed: u64, seat: PlayerId, level: BotLevel) -> BotBrain {
        BotBrain {
            brain: Some(brain::brain(level.brain(), seed, seat, true)),
            level,
            turn: None,
            answers: 0,
        }
    }

    /// Whether the brain searches the table, so the bot fetches a copy of the engine before it answers.
    pub fn reads_table(&self) -> bool {
        self.level != BotLevel::Easy
    }

    /// Notes the turn the table is in, which starts a new count of answers when it changes.
    pub fn turn_is(&mut self, turn: u32) {
        if self.turn != Some(turn) {
            self.turn = Some(turn);
            self.answers = 0;
        }
    }

    /// The answer to `decision`, pending in `table` when the bot has it; a pass at priority once the turn has had its
    /// answers.
    pub fn answer(&mut self, decision: &Decision, table: Option<&Engine>) -> Answer {
        self.answers += 1;
        if matches!(decision.kind, DecisionKind::Priority(_)) && self.answers > TURN_ANSWERS {
            return Answer::Priority(PriorityAction::Pass);
        }
        match self.brain.as_mut() {
            Some(brain) => brain.choose(decision, table),
            None => fallback(decision).unwrap_or(Answer::Priority(PriorityAction::Pass)),
        }
    }

    /// [`BotBrain::answer`] on a blocking thread in one of `thinkers`' slots, so a search never holds up the runtime
    /// and the server's searches stay capped; `None` if that thread failed.
    async fn think(
        &mut self,
        decision: Decision,
        table: Option<Box<Engine>>,
        thinkers: &Thinkers,
    ) -> Option<Answer> {
        let brain = self.brain.take()?;
        let mut alone = BotBrain {
            brain: Some(brain),
            level: self.level,
            turn: self.turn,
            answers: self.answers,
        };
        let (alone, answer) = thinkers
            .think(move |share| {
                if let Some(brain) = alone.brain.as_mut() {
                    brain.set_budget(share);
                }
                let answer = alone.answer(&decision, table.as_deref());
                (alone, answer)
            })
            .await?;
        self.brain = alone.brain;
        self.answers = alone.answers;
        Some(answer)
    }
}

/// The answer that decides least, for a decision whose own answer was refused; none when nothing can be given.
pub fn fallback(decision: &Decision) -> Option<Answer> {
    crate::pace::fallback(decision)
}

/// A decision the bot holds and when it answers it.
struct Waiting {
    id: DecisionId,
    decision: Decision,
    due: Instant,
    /// Whether the answer sent for it was refused once already.
    refused: bool,
}

/// Plays the seat of `bot` in `handle` at its level until the game ends or leaves the registry (`removed` changes or
/// closes): joins the seat (again whenever its connection is dropped), says it is ready (again whenever the lobby shows it is
/// not), and answers each decision after the pace's delay with [`BotBrain`], thinking in the pace's slots. A refused
/// answer is followed by the table's fallback answer, and when that is refused too, or there is
/// none, the bot concedes, so a bot never stalls a game, and never keeps alive a game the registry let go.
pub fn spawn_bot_seat(
    handle: GameHandle,
    bot: BotSeat,
    seed: u64,
    pace: BotPace,
    mut removed: watch::Receiver<Removal>,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        tokio::select! {
            () = run_seat(handle, bot, seed, pace) => {}
            // A change or a dropped sender both mean the game left the registry.
            _ = removed.changed() => {}
        }
    })
}

/// The body of [`spawn_bot_seat`]: joins and plays until the game ends or closes.
async fn run_seat(handle: GameHandle, bot: BotSeat, seed: u64, pace: BotPace) {
    let seat = bot.seat;
    {
        let mut brain = BotBrain::new(seed, seat, bot.level);
        loop {
            let Ok(mut inbox) = handle.join(seat).await else {
                return;
            };
            // In a lobby this readies the seat; in a running game the table refuses it, which changes nothing.
            if handle.send(seat, ClientMessage::Ready).await.is_err() {
                return;
            }
            match play(&handle, seat, &mut brain, &mut inbox, &pace).await {
                Ended::Over => return,
                Ended::Dropped => sleep(REJOIN_AFTER).await,
            }
        }
    }
}

/// [`spawn_bot_seat`] for each of `seats`.
pub(crate) fn spawn_bot_seats(
    handle: &GameHandle,
    seats: &[BotSeat],
    seed: u64,
    pace: &BotPace,
    removed: &watch::Receiver<Removal>,
) {
    for bot in seats {
        spawn_bot_seat(handle.clone(), *bot, seed, pace.clone(), removed.clone());
    }
}

/// How the server's bots answer: the wait before each answer and the slots they think in.
#[derive(Clone, Debug)]
pub struct BotPace {
    /// How long a bot waits before it answers ([`BOT_DELAY`]).
    pub delay: Duration,
    /// The server's thinking slots.
    pub thinkers: Thinkers,
}

/// How a run of [`play`] ended.
enum Ended {
    /// The game ended or closed.
    Over,
    /// The seat's connection was dropped (a full queue); the bot joins again.
    Dropped,
}

/// Answers the decisions that arrive on `inbox` until the game ends or the connection drops.
async fn play(
    handle: &GameHandle,
    seat: PlayerId,
    brain: &mut BotBrain,
    inbox: &mut tokio::sync::mpsc::Receiver<ServerMessage>,
    pace: &BotPace,
) -> Ended {
    let mut waiting: Option<Waiting> = None;
    loop {
        let message = match &waiting {
            Some(held) => {
                tokio::select! {
                    biased;
                    message = inbox.recv() => message,
                    () = sleep_until(held.due) => {
                        let Some(held) = waiting.take() else { continue };
                        let table = if brain.reads_table() {
                            handle.table().await.ok().flatten()
                        } else {
                            None
                        };
                        let thought = brain.think(held.decision.clone(), table, &pace.thinkers).await;
                        let Some(answer) = thought else {
                            return Ended::Over;
                        };
                        if send_answer(handle, seat, held.id, answer).await.is_err() {
                            return Ended::Over;
                        }
                        waiting = Some(Waiting { due: far_future(), ..held });
                        continue;
                    }
                }
            }
            None => inbox.recv().await,
        };
        let Some(message) = message else {
            return Ended::Dropped;
        };
        match message {
            ServerMessage::Decision { id, decision, .. } if decision.player == seat => {
                waiting = Some(Waiting {
                    id,
                    decision,
                    due: Instant::now() + pace.delay,
                    refused: false,
                });
            }
            ServerMessage::Events(batch) => brain.turn_is(batch.table.turn.number),
            ServerMessage::Snapshot { state, .. } => brain.turn_is(state.table.turn.number),
            // A change of teams takes every Ready back; the bot says it again so the lobby can still start.
            ServerMessage::Lobby { seats, .. }
                if seats.iter().any(|lobby| lobby.seat == seat && !lobby.ready) =>
            {
                if handle.send(seat, ClientMessage::Ready).await.is_err() {
                    return Ended::Over;
                }
            }
            ServerMessage::Error {
                code: ErrorCode::IllegalAnswer,
                ..
            } => {
                let Some(held) = waiting.take() else { continue };
                let fallback = fallback(&held.decision).filter(|_| !held.refused);
                let Some(answer) = fallback else {
                    let _ = handle.send(seat, ClientMessage::Concede).await;
                    return Ended::Over;
                };
                if send_answer(handle, seat, held.id, answer).await.is_err() {
                    return Ended::Over;
                }
                waiting = Some(Waiting {
                    refused: true,
                    due: far_future(),
                    ..held
                });
            }
            ServerMessage::GameOver { .. } => return Ended::Over,
            _ => {}
        }
    }
}

/// Sends `answer` to decision `id` for `seat`.
async fn send_answer(
    handle: &GameHandle,
    seat: PlayerId,
    id: DecisionId,
    answer: Answer,
) -> Result<(), crate::handle::GameClosed> {
    handle
        .send(
            seat,
            ClientMessage::Answer {
                decision: id,
                answer,
            },
        )
        .await
}

/// A time that never comes: an answered decision waits only for the table's reply.
fn far_future() -> Instant {
    Instant::now() + Duration::from_secs(60 * 60 * 24 * 365)
}

#[cfg(test)]
mod tests {
    //! Goal: an Easy bot plays a land whenever it can, and every bot after [`TURN_ANSWERS`] answers in one turn only passes until the turn changes, so a loop of free plays cannot hold the table; only a searching bot fetches the table, a stored bot seat reads back with its level (a bare seat as Easy), and the thinking slots cap the bots searching at once and shrink their budgets while others wait.
    #![allow(clippy::unwrap_used)]
    use mtg_core::ids::ObjectId;
    use mtg_engine::decision::PriorityOptions;
    use rstest::rstest;

    use super::*;

    const SEAT: PlayerId = PlayerId(1);

    fn priority(lands: Vec<ObjectId>, castable: Vec<ObjectId>) -> Decision {
        Decision {
            player: SEAT,
            kind: DecisionKind::Priority(Box::new(PriorityOptions {
                playable_lands: lands,
                castable,
                mana_abilities: Vec::new(),
                abilities: Vec::new(),
                suspendable: Vec::new(),
                foretellable: Vec::new(),
                plottable: Vec::new(),
                companion: false,
                turnable: Vec::new(),
                unlockable: Vec::new(),
                endable: Vec::new(),
                faces: Vec::new(),
                unplayable: Vec::new(),
                inactive: Vec::new(),
                can_act: true,
                can_pass: true,
            })),
        }
    }

    #[rstest]
    #[case::a_land_alone(vec![ObjectId(4)], vec![])]
    #[case::a_land_and_a_spell(vec![ObjectId(4), ObjectId(5)], vec![ObjectId(6)])]
    fn a_land_comes_first(#[case] lands: Vec<ObjectId>, #[case] castable: Vec<ObjectId>) {
        let mut brain = BotBrain::new(7, SEAT, BotLevel::Easy);
        let answer = brain.answer(&priority(lands, castable), None);
        assert_eq!(
            answer,
            Answer::Priority(PriorityAction::PlayLand(ObjectId(4)))
        );
    }

    #[rstest]
    fn it_only_passes_once_a_turn_has_had_its_answers(#[values(1, 2, 3)] seed: u64) {
        let mut brain = BotBrain::new(seed, SEAT, BotLevel::Easy);
        brain.turn_is(3);
        let busy = priority(vec![ObjectId(4)], vec![ObjectId(6)]);
        for _ in 0..TURN_ANSWERS {
            brain.answer(&busy, None);
        }
        assert_eq!(
            brain.answer(&busy, None),
            Answer::Priority(PriorityAction::Pass)
        );
        brain.turn_is(3);
        assert_eq!(
            brain.answer(&busy, None),
            Answer::Priority(PriorityAction::Pass)
        );
        brain.turn_is(4);
        assert_eq!(
            brain.answer(&busy, None),
            Answer::Priority(PriorityAction::PlayLand(ObjectId(4)))
        );
    }

    #[rstest]
    #[case::easy(BotLevel::Easy, false)]
    #[case::normal(BotLevel::Normal, true)]
    #[case::hard(BotLevel::Hard, true)]
    #[case::very_hard(BotLevel::VeryHard, true)]
    fn only_a_searching_bot_reads_the_table(#[case] level: BotLevel, #[case] reads: bool) {
        assert_eq!(BotBrain::new(1, SEAT, level).reads_table(), reads);
    }

    #[rstest]
    #[case::bare_seat(serde_json::json!(2), BotLevel::Easy)]
    #[case::leveled(serde_json::json!({"seat": 2, "level": "hard"}), BotLevel::Hard)]
    #[case::very_hard(serde_json::json!({"seat": 2, "level": "very_hard"}), BotLevel::VeryHard)]
    fn a_stored_bot_seat_reads_back(#[case] stored: serde_json::Value, #[case] level: BotLevel) {
        let bot: BotSeat = serde_json::from_value(stored).unwrap();
        assert_eq!(
            bot,
            BotSeat {
                seat: PlayerId(2),
                level
            }
        );
    }

    #[rstest]
    fn a_longer_queue_leaves_a_smaller_share(#[values(0, 1, 3, 10, 1000)] queued: usize) {
        assert!(share_for(queued + 1) <= share_for(queued));
        assert!(share_for(queued) >= LEAST_SHARE);
        assert!((share_for(0) - 1.0).abs() < f64::EPSILON);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn thinkers_cap_the_bots_thinking_at_once() {
        let thinkers = Thinkers::new(2);
        let running = Arc::new(AtomicUsize::new(0));
        let most = Arc::new(AtomicUsize::new(0));
        let mut tasks = Vec::new();
        for _ in 0..6 {
            let thinkers = thinkers.clone();
            let running = Arc::clone(&running);
            let most = Arc::clone(&most);
            tasks.push(tokio::spawn(async move {
                thinkers
                    .think(move |share| {
                        let now = running.fetch_add(1, Ordering::SeqCst) + 1;
                        most.fetch_max(now, Ordering::SeqCst);
                        std::thread::sleep(Duration::from_millis(40));
                        running.fetch_sub(1, Ordering::SeqCst);
                        share
                    })
                    .await
                    .unwrap()
            }));
        }
        let mut shares = Vec::new();
        for task in tasks {
            shares.push(task.await.unwrap());
        }
        assert!(most.load(Ordering::SeqCst) <= 2);
        assert!(shares.iter().any(|share| *share < 1.0), "{shares:?}");
    }
}
