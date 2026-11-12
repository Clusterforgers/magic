//! Bot seats: a server-side driver that plays a seat of a game the way a client would, through its `GameHandle`.

use std::time::Duration;

use mtg_core::ids::PlayerId;
use mtg_engine::Decision;
use mtg_engine::decision::{Answer, DecisionKind, PriorityAction};
use mtg_protocol::{ClientMessage, DecisionId, ErrorCode, ServerMessage};
use mtg_testkit::RandomBot;
use tokio::sync::watch;
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

/// What the bot of one seat decides with: random legal answers, a land whenever it can play one, and a pass once it has
/// answered [`TURN_ANSWERS`] times in a turn.
#[derive(Debug, Clone)]
pub struct BotBrain {
    random: RandomBot,
    turn: Option<u32>,
    answers: u32,
}

impl BotBrain {
    /// The brain of `seat` in a game shuffled with `seed`: its random choices replay with the seed.
    pub fn new(seed: u64, seat: PlayerId) -> BotBrain {
        BotBrain {
            random: RandomBot::new(seed, 0xB07 + seat.index() as u64),
            turn: None,
            answers: 0,
        }
    }

    /// Notes the turn the table is in, which starts a new count of answers when it changes.
    pub fn turn_is(&mut self, turn: u32) {
        if self.turn != Some(turn) {
            self.turn = Some(turn);
            self.answers = 0;
        }
    }

    /// The answer to `decision`.
    pub fn answer(&mut self, decision: &Decision) -> Answer {
        self.answers += 1;
        if let DecisionKind::Priority(options) = &decision.kind {
            if self.answers > TURN_ANSWERS {
                return Answer::Priority(PriorityAction::Pass);
            }
            if let Some(land) = options.playable_lands.first() {
                return Answer::Priority(PriorityAction::PlayLand(*land));
            }
        }
        self.random.choose(decision)
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

/// Plays `seat` of `handle` until the game ends or leaves the registry (`removed` changes or closes): joins the seat
/// (again whenever its connection is dropped), says it is ready, and answers each decision after `delay` with
/// [`BotBrain`]. A refused answer is followed by the table's fallback answer, and when that is refused too, or there is
/// none, the bot concedes, so a bot never stalls a game, and never keeps alive a game the registry let go.
pub fn spawn_bot_seat(
    handle: GameHandle,
    seat: PlayerId,
    seed: u64,
    delay: Duration,
    mut removed: watch::Receiver<Removal>,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        tokio::select! {
            () = run_seat(handle, seat, seed, delay) => {}
            // A change or a dropped sender both mean the game left the registry.
            _ = removed.changed() => {}
        }
    })
}

/// The body of [`spawn_bot_seat`]: joins and plays until the game ends or closes.
async fn run_seat(handle: GameHandle, seat: PlayerId, seed: u64, delay: Duration) {
    {
        let mut brain = BotBrain::new(seed, seat);
        loop {
            let Ok(mut inbox) = handle.join(seat).await else {
                return;
            };
            // In a lobby this readies the seat; in a running game the table refuses it, which changes nothing.
            if handle.send(seat, ClientMessage::Ready).await.is_err() {
                return;
            }
            match play(&handle, seat, &mut brain, &mut inbox, delay).await {
                Ended::Over => return,
                Ended::Dropped => sleep(REJOIN_AFTER).await,
            }
        }
    }
}

/// [`spawn_bot_seat`] for each of `seats`.
pub(crate) fn spawn_bot_seats(
    handle: &GameHandle,
    seats: &[PlayerId],
    seed: u64,
    delay: Duration,
    removed: &watch::Receiver<Removal>,
) {
    for seat in seats {
        spawn_bot_seat(handle.clone(), *seat, seed, delay, removed.clone());
    }
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
    delay: Duration,
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
                        let answer = brain.answer(&held.decision);
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
                    due: Instant::now() + delay,
                    refused: false,
                });
            }
            ServerMessage::Events(batch) => brain.turn_is(batch.table.turn.number),
            ServerMessage::Snapshot { state, .. } => brain.turn_is(state.table.turn.number),
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
    //! Goal: a bot plays a land whenever it can, answers every kind of decision with a legal-looking answer, and after [`TURN_ANSWERS`] answers in one turn only passes until the turn changes, so a loop of free plays cannot hold the table.
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
                can_act: true,
                can_pass: true,
            })),
        }
    }

    #[rstest]
    #[case::a_land_alone(vec![ObjectId(4)], vec![])]
    #[case::a_land_and_a_spell(vec![ObjectId(4), ObjectId(5)], vec![ObjectId(6)])]
    fn a_land_comes_first(#[case] lands: Vec<ObjectId>, #[case] castable: Vec<ObjectId>) {
        let mut brain = BotBrain::new(7, SEAT);
        let answer = brain.answer(&priority(lands, castable));
        assert_eq!(
            answer,
            Answer::Priority(PriorityAction::PlayLand(ObjectId(4)))
        );
    }

    #[rstest]
    fn it_only_passes_once_a_turn_has_had_its_answers(#[values(1, 2, 3)] seed: u64) {
        let mut brain = BotBrain::new(seed, SEAT);
        brain.turn_is(3);
        let busy = priority(vec![ObjectId(4)], vec![ObjectId(6)]);
        for _ in 0..TURN_ANSWERS {
            brain.answer(&busy);
        }
        assert_eq!(brain.answer(&busy), Answer::Priority(PriorityAction::Pass));
        brain.turn_is(3);
        assert_eq!(brain.answer(&busy), Answer::Priority(PriorityAction::Pass));
        brain.turn_is(4);
        assert_eq!(
            brain.answer(&busy),
            Answer::Priority(PriorityAction::PlayLand(ObjectId(4)))
        );
    }
}
