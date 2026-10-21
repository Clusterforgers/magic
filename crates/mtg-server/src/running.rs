//! A started game as a plain state machine: the engine plus the decision ids that tell stale answers from current ones.

use std::time::Duration;

use mtg_core::ids::PlayerId;
use mtg_engine::forecast::CombatForecast;
use mtg_engine::state::Stage;
use mtg_engine::{Answer, Checkpoint, Decision, Engine, EngineError, Event, GameState};
use mtg_protocol::{DecisionId, ErrorCode, ServerMessage, StateView};
use tokio::time::Instant;

use crate::dwell::Waits;
use crate::pace::{self, Pace};
use crate::stops::{self, Stops};
use crate::windows::{self, Verdict};
use crate::yields::Yields;

/// Most priority passes made for idle seats in one go; past it the decision is sent as it is.
///
/// A real game needs a handful. The cap keeps one batch small and the actor responsive if a table
/// ever offers nothing to do for dozens of turns.
const MAX_IDLE_PASSES: usize = 500;

/// A client message that was refused, as the `Error` it becomes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Rejection {
    pub(crate) code: ErrorCode,
    pub(crate) message: String,
}

impl Rejection {
    pub(crate) fn new(code: ErrorCode, message: impl Into<String>) -> Rejection {
        Rejection {
            code,
            message: message.into(),
        }
    }

    pub(crate) fn into_message(self) -> ServerMessage {
        ServerMessage::Error {
            code: self.code,
            message: self.message,
        }
    }

    fn engine_failed() -> Rejection {
        Rejection::new(
            ErrorCode::Internal,
            "the game engine failed; the game is unchanged",
        )
    }

    fn stale() -> Rejection {
        Rejection::new(
            ErrorCode::StaleDecision,
            "that decision is not pending for you",
        )
    }
}

/// One engine step taken in memory but not yet persisted or announced.
#[derive(Debug)]
pub(crate) struct Step {
    /// The state before the step, which event projection needs.
    pub(crate) before: GameState,
    /// Undoes the step if it cannot be persisted.
    pub(crate) checkpoint: Checkpoint,
    /// The events the step produced.
    pub(crate) events: Vec<Event>,
    /// Log position of the first event.
    pub(crate) from_seq: u64,
    /// The seat whose message caused the step.
    pub(crate) origin: PlayerId,
    /// Whether a pending decision the step left unchanged keeps its id.
    pub(crate) keep_unchanged: bool,
    /// Whether the step is the origin seat's pass of priority (its own, or the table's at its clock or dwell).
    pub(crate) passed: bool,
}

/// The decision awaiting an answer, the id the client must echo and when its clock runs out.
#[derive(Debug)]
struct Pending {
    id: DecisionId,
    decision: Decision,
    /// When the server answers for the seat; `None` for a decision without a clock.
    due: Option<Instant>,
    /// Whether the decision is held back for the seat's dwell: never sent, and the table passes for the seat at `due`.
    hidden: bool,
}

/// A decision just issued: what its seat is sent.
#[derive(Debug)]
pub(crate) struct Issued {
    pub(crate) id: DecisionId,
    pub(crate) decision: Decision,
    /// Milliseconds on its clock, if it has one.
    pub(crate) deadline_ms: Option<u64>,
}

/// Where a run of passes stopped.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Halt {
    /// At a decision that its seat is sent, at one that is cut short, or at the end of the game.
    Ask,
    /// At a priority window of a seat that cannot act: the table passes for it once its dwell is over.
    Dwell,
}

/// A started game.
#[derive(Debug)]
pub(crate) struct Running {
    pub(crate) engine: Engine,
    pending: Option<Pending>,
    last_id: u64,
    pace: Pace,
    /// The dwells of the idle windows, drawn apart from the game.
    waits: Waits,
}

pub(crate) fn count(len: usize) -> u64 {
    u64::try_from(len).unwrap_or(u64::MAX)
}

/// Whole milliseconds, saturating.
fn millis(span: Duration) -> u64 {
    u64::try_from(span.as_millis()).unwrap_or(u64::MAX)
}

impl Running {
    /// Wraps `engine`; call [`settle`](Running::settle) to issue the first decision id.
    pub(crate) fn new(engine: Engine, pace: Pace) -> Running {
        Running {
            engine,
            pending: None,
            last_id: 0,
            pace,
            waits: Waits::fresh(pace.dwell),
        }
    }

    /// The seat that must answer, if any.
    pub(crate) fn waiting_on(&self) -> Option<PlayerId> {
        self.pending.as_ref().map(|p| p.decision.player)
    }

    /// When the pending decision's clock runs out, if it has one.
    pub(crate) fn due(&self) -> Option<Instant> {
        self.pending.as_ref().and_then(|p| p.due)
    }

    /// The seat and kind of the pending decision when it was sent (so its clock is a response clock, not a dwell).
    pub(crate) fn sent_decision(&self) -> Option<(PlayerId, &'static str)> {
        let pending = self.pending.as_ref().filter(|p| !p.hidden)?;
        Some((pending.decision.player, pending.decision.kind.name()))
    }

    /// Moves the clock of the pending decision to `due` (nothing happens for a decision without one).
    pub(crate) fn postpone(&mut self, due: Instant) {
        if let Some(pending) = self.pending.as_mut().filter(|p| p.due.is_some()) {
            pending.due = Some(due);
        }
    }

    /// Drops the clock of the pending decision, so a decision the server cannot answer for is not retried forever.
    pub(crate) fn stop_clock(&mut self) {
        if let Some(pending) = self.pending.as_mut() {
            pending.due = None;
        }
    }

    /// What the table does with `decision` under `stops` and `yields`: with the dwell switched off, a pass after a
    /// dwell is a pass at once.
    fn verdict(&self, decision: &Decision, stops: &Stops, yields: &mut Yields) -> Verdict {
        match windows::verdict(decision, stops, yields, self.engine.state()) {
            Verdict::Dwell if self.pace.dwell.is_off() => Verdict::Pass,
            other => other,
        }
    }

    /// What the table does with the pending decision if it is `seat`'s, under `stops` and `yields`.
    ///
    /// Nothing is changed: a yield the decision would end stays in `yields`.
    pub(crate) fn verdict_pending(
        &self,
        seat: PlayerId,
        stops: &Stops,
        yields: &Yields,
    ) -> Option<Verdict> {
        let pending = self.pending.as_ref().filter(|p| p.decision.player == seat);
        pending.map(|p| self.verdict(&p.decision, stops, &mut yields.clone()))
    }

    /// Whether the pending decision is `seat`'s and the table does not ask it under `stops` and `yields`: it passes
    /// for the seat, at once or after a dwell.
    pub(crate) fn passes_pending(&self, seat: PlayerId, stops: &Stops, yields: &Yields) -> bool {
        self.verdict_pending(seat, stops, yields)
            .is_some_and(|verdict| verdict != Verdict::Ask)
    }

    /// Whether the pending decision is `seat`'s, is held back for its dwell and is one that `stops` and `yields`
    /// now ask the seat: its dwell is over and it has to be sent.
    pub(crate) fn dwell_ends(&self, seat: PlayerId, stops: &Stops, yields: &Yields) -> bool {
        let held = self
            .pending
            .as_ref()
            .is_some_and(|p| p.hidden && p.decision.player == seat);
        held && self.verdict_pending(seat, stops, yields) == Some(Verdict::Ask)
    }

    /// Aligns the pending decision with the engine's and returns what a client must be sent, if anything.
    ///
    /// Every new decision gets a fresh id and, when [`Pace`] gives it one, a clock that starts now. With
    /// `keep_unchanged`, a decision that did not change keeps its id and clock and is not sent again, so a
    /// concession elsewhere does not invalidate a pending answer.
    ///
    /// At `Halt::Dwell` the decision is held back instead of sent: it gets no clock but a dwell of its own
    /// ([`Waits::wait`], drawn now, whatever the windows before it were), and the actor passes for the seat at that
    /// time. A decision that is held back and unchanged keeps its wait.
    pub(crate) fn settle(&mut self, keep_unchanged: bool, halt: Halt) -> Option<Issued> {
        let Some(decision) = self.engine.decision().cloned() else {
            self.pending = None;
            return None;
        };
        let dwell = halt == Halt::Dwell;
        let kept = self
            .pending
            .as_ref()
            .is_some_and(|p| p.decision == decision && p.hidden == dwell);
        if keep_unchanged && kept {
            return None;
        }
        self.last_id += 1;
        let id = DecisionId(self.last_id);
        if dwell {
            let wait = self.waits.wait();
            let now = Instant::now();
            self.pending = Some(Pending {
                id,
                decision,
                due: Some(now.checked_add(wait).unwrap_or(now)),
                hidden: true,
            });
            return None;
        }
        let state = self.engine.state();
        let clock = if state.stage() == Stage::Mulligan {
            self.pace.opening_clock()
        } else {
            self.pace.clock_for(&decision, state.turn().active())
        };
        let due = clock.and_then(|span| Instant::now().checked_add(span));
        self.pending = Some(Pending {
            id,
            decision: decision.clone(),
            due,
            hidden: false,
        });
        Some(Issued {
            id,
            decision,
            deadline_ms: clock.map(millis),
        })
    }

    /// What a client needs to catch up: a snapshot, its pending decision and the result if the game is over.
    ///
    /// The decision carries the time its clock has left, not the time it started with. One that is held back for its
    /// dwell is not sent: the seat is passed for, and a snapshot taken meanwhile must not tell it so.
    pub(crate) fn sync_messages(&self, seat: PlayerId) -> Vec<ServerMessage> {
        let mut messages = vec![ServerMessage::Snapshot {
            seq: count(self.engine.events().len()),
            state: Box::new(StateView::for_player(self.engine.state(), seat)),
        }];
        let visible = self
            .pending
            .as_ref()
            .filter(|p| p.decision.player == seat && !p.hidden);
        if let Some(pending) = visible {
            let now = Instant::now();
            messages.push(ServerMessage::Decision {
                id: pending.id,
                decision: pending.decision.clone(),
                deadline_ms: pending
                    .due
                    .map(|due| millis(due.saturating_duration_since(now))),
            });
        }
        if let Some(result) = self.engine.result() {
            messages.push(ServerMessage::GameOver { result });
        }
        messages
    }

    /// Applies `seat`'s answer to the decision `id`, if that decision is pending for that seat.
    ///
    /// A decision held back for its dwell was never sent, so no client is answering it: that is stale too.
    pub(crate) fn answer(
        &mut self,
        seat: PlayerId,
        id: DecisionId,
        answer: Answer,
    ) -> Result<Step, Rejection> {
        if self.pending.as_ref().is_none_or(|p| p.hidden) {
            return Err(Rejection::stale());
        }
        if pace::is_removal(&answer) && self.is_pending_for(seat, id) {
            return Err(Rejection::new(
                ErrorCode::IllegalAnswer,
                "only the clock removes a triggered ability",
            ));
        }
        self.apply_answer(seat, id, answer)
    }

    /// Whether the decision `id` is the pending one and is `seat`'s.
    fn is_pending_for(&self, seat: PlayerId, id: DecisionId) -> bool {
        self.pending
            .as_ref()
            .is_some_and(|p| p.id == id && p.decision.player == seat)
    }

    /// [`answer`](Running::answer) for the pending decision whether or not a client was sent it.
    fn apply_answer(
        &mut self,
        seat: PlayerId,
        id: DecisionId,
        answer: Answer,
    ) -> Result<Step, Rejection> {
        if !self.is_pending_for(seat, id) {
            return Err(Rejection::stale());
        }
        let passes = answer == pace::pass();
        self.step(seat, false, |engine| engine.apply(answer))
            .map(|step| Step {
                passed: passes,
                ..step
            })
            .map_err(|error| match error {
                EngineError::Illegal(illegal) => {
                    Rejection::new(ErrorCode::IllegalAnswer, illegal.to_string())
                }
                EngineError::NoDecision => {
                    Rejection::new(ErrorCode::StaleDecision, "no decision is pending")
                }
                _ => Rejection::engine_failed(),
            })
    }

    /// The forecast of `answer` to the decision `id`, only if that decision is pending for `seat` right now and the
    /// engine can preview the answer; nothing is logged and the engine is not changed.
    pub(crate) fn preview(
        &self,
        seat: PlayerId,
        id: DecisionId,
        answer: &Answer,
    ) -> Option<CombatForecast> {
        let current = self
            .pending
            .as_ref()
            .is_some_and(|p| p.id == id && p.decision.player == seat && !p.hidden);
        if !current {
            return None;
        }
        self.engine.preview_forecast(seat, answer).ok()
    }

    /// Answers the pending decision for its seat with the safe default: a pass, or no blocks.
    ///
    /// It goes through the same checks, log and projection as [`answer`](Running::answer), so it is handled like the
    /// seat's own move, whether the decision was sent to the seat or held back for its dwell.
    pub(crate) fn fall_back(&mut self) -> Result<Step, Rejection> {
        let pending = self.pending.as_ref();
        let Some((id, seat, answer)) = pending.and_then(|p| {
            pace::fallback(&p.decision).map(|answer| (p.id, p.decision.player, answer))
        }) else {
            return Err(Rejection::new(
                ErrorCode::StaleDecision,
                "no decision is pending that can be answered for its seat",
            ));
        };
        self.apply_answer(seat, id, answer)
    }

    /// Passes for every seat the windows pass for at once ([`windows::verdict`]: not in Full Control, not at a window
    /// of its own, or yielding, or at a window that is not one of the default table) until some decision needs a
    /// player or the pass has to wait.
    ///
    /// The passes go through the engine like any answer; their events are added to `events`, so the
    /// whole stretch is saved and shown as one batch. A seat that is asked, or is asked anything else, stops the
    /// run, and so does the end of the game; so does a seat that must wait at its window (`Window::dwells`) while the
    /// dwell is on, with `Halt::Dwell`: nothing is passed for it here, the actor does that when its dwell is over. A yield
    /// that reaches its end on the way is dropped from `yields`, and the one-turn stops of the steps the passes
    /// leave are cleared from `stops`; the caller keeps both only if the stretch is saved.
    pub(crate) fn skip_idle(
        &mut self,
        events: &mut Vec<Event>,
        stops: &mut Stops,
        yields: &mut Yields,
    ) -> Result<Halt, EngineError> {
        self.skip_up_to(MAX_IDLE_PASSES, events, stops, yields)
    }

    /// [`skip_idle`](Running::skip_idle) with a limit on the passes, which a test can make small.
    fn skip_up_to(
        &mut self,
        limit: usize,
        events: &mut Vec<Event>,
        stops: &mut Stops,
        yields: &mut Yields,
    ) -> Result<Halt, EngineError> {
        for _ in 0..limit {
            let Some(decision) = self.engine.decision() else {
                return Ok(Halt::Ask);
            };
            match self.verdict(decision, stops, yields) {
                Verdict::Ask => return Ok(Halt::Ask),
                Verdict::Dwell => return Ok(Halt::Dwell),
                Verdict::Pass => {}
            }
            let at = stops::place(self.engine.state());
            let passed = self.engine.apply(pace::pass())?;
            stops.left(at, &passed);
            events.extend(passed);
        }
        // Cut short: the next priority decision is sent as it is, so a yield it belongs to ends here instead of
        // leaving a seat that "passes for it" with a prompt to answer.
        if let Some(decision) = self.engine.decision().filter(|d| pace::is_priority(d)) {
            yields.cancel(decision.player);
        }
        Ok(Halt::Ask)
    }

    /// Whether the pending decision is a priority decision of `seat`.
    pub(crate) fn asks_priority_of(&self, seat: PlayerId) -> bool {
        self.pending
            .as_ref()
            .is_some_and(|p| p.decision.player == seat && pace::is_priority(&p.decision))
    }

    /// A step that changes nothing, so that the pending decision of `seat` meets a new yield and, if the
    /// yield does not pass for it, is issued again with a new id.
    pub(crate) fn restart_decision(&mut self, seat: PlayerId) -> Result<Step, Rejection> {
        self.step(seat, false, |_| Ok(Vec::new()))
            .map_err(|_| Rejection::engine_failed())
    }

    /// Removes `seat` from the game.
    pub(crate) fn concede(&mut self, seat: PlayerId) -> Result<Step, Rejection> {
        self.step(seat, true, |engine| engine.concede(seat))
            .map_err(|error| match error {
                EngineError::NoDecision => {
                    Rejection::new(ErrorCode::BadMessage, "the game is already over")
                }
                EngineError::NotInGame(_) => {
                    Rejection::new(ErrorCode::BadMessage, "you are not in the game")
                }
                _ => Rejection::engine_failed(),
            })
    }

    fn step(
        &mut self,
        origin: PlayerId,
        keep_unchanged: bool,
        run: impl FnOnce(&mut Engine) -> Result<Vec<Event>, EngineError>,
    ) -> Result<Step, EngineError> {
        let before = self.engine.state().clone();
        let checkpoint = self.engine.checkpoint();
        let events = run(&mut self.engine)?;
        let from_seq = count(self.engine.events().len() - events.len());
        Ok(Step {
            before,
            checkpoint,
            events,
            from_seq,
            origin,
            keep_unchanged,
            passed: false,
        })
    }
}

#[cfg(test)]
mod tests {
    //! Goal: a run of passes stops at its limit with the next priority decision still pending, and a yield that decision belongs to ends there, so the seat is never handed a prompt while the server still "passes for it"; every other seat is left alone; a seat that cannot act at a window where it waits (a spell on the stack, not its own empty main phase) stops the run with its decision held back (never sent, answered or previewed, passed for by the table with its dwell as the clock) until the rules ask it.
    use super::*;
    use crate::dwell::Dwell;
    use mtg_engine::DecisionKind;
    use mtg_engine::decision::{PayChoice, PriorityAction};
    use mtg_protocol::{FullControlMode, YieldUntil};
    use mtg_testkit::Scenario;
    use mtg_testkit::cards::free_creature;
    use rstest::rstest;

    const SEATS: [PlayerId; 2] = [PlayerId(0), PlayerId(1)];

    fn running() -> Running {
        let setup = Scenario::new(2).setup().unwrap();
        let mut running = Running::new(Engine::new(setup).unwrap(), Pace::MANUAL);
        running.settle(false, Halt::Ask);
        running
    }

    fn yielding(running: &Running, seats: &[PlayerId]) -> Yields {
        let mut yields = Yields::new(2);
        for seat in seats {
            yields.start(*seat, YieldUntil::EndOfTurn, running.engine.state());
        }
        yields
    }

    #[rstest]
    #[case::the_cut_lands_on_a_seat_that_yields(2, &[SEATS[0], SEATS[1]], &[SEATS[1]])]
    #[case::the_cut_lands_on_a_seat_that_does_not(1, &[SEATS[0]], &[SEATS[0]])]
    fn the_cut_ends_the_yield_of_the_seat_whose_decision_is_next(
        #[case] limit: usize,
        #[case] before: &[PlayerId],
        #[case] after: &[PlayerId],
    ) {
        let mut running = running();
        let mut yields = yielding(&running, before);
        let mut events = Vec::new();
        running
            .skip_up_to(limit, &mut events, &mut Stops::new(2), &mut yields)
            .unwrap();
        let passes = events
            .iter()
            .filter(|event| matches!(event, Event::PriorityPassed { .. }))
            .count();
        assert_eq!(passes, limit, "one pass for each of the limit");
        let held: Vec<PlayerId> = SEATS
            .into_iter()
            .filter(|seat| yields.get(*seat).is_some())
            .collect();
        assert_eq!(held, after);
        assert!(
            running.engine.decision().is_some(),
            "the decision is still there to be sent"
        );
    }

    #[test]
    fn without_a_limit_in_the_way_the_run_goes_on_until_a_seat_must_decide() {
        let mut running = running();
        let mut yields = yielding(&running, &[SEATS[0]]);
        let mut stops = Stops::new(2);
        stops.set_full_control(SEATS[1], FullControlMode::Held);
        let mut events = Vec::new();
        running
            .skip_idle(&mut events, &mut stops, &mut yields)
            .unwrap();
        assert_eq!(events.len(), 1, "seat 1 does not yield and is asked");
        assert_eq!(running.engine.decision().map(|d| d.player), Some(SEATS[1]));
        assert_eq!(yields.get(SEATS[0]), Some(YieldUntil::EndOfTurn));
    }

    /// Seat 0 in its first main phase after it cast its free creature: the spell waits on the stack and seat 0 holds
    /// nothing else, so it cannot act at a window where a seat that cannot act waits.
    fn idle_with_a_spell(dwell: Dwell) -> Running {
        let pace = Pace {
            dwell,
            ..Pace::MANUAL
        };
        let setup = Scenario::new(2)
            .hand(0, [free_creature("Spare Bear", 2, 2)])
            .setup()
            .unwrap();
        let mut engine = Engine::new(setup).unwrap();
        let Some(Decision {
            kind: DecisionKind::Priority(options),
            ..
        }) = engine.decision().cloned()
        else {
            panic!("seat 0 holds priority");
        };
        let spell = options.castable[0];
        engine
            .apply(Answer::Priority(PriorityAction::CastSpell(spell)))
            .unwrap();
        engine.apply(Answer::PayMana(PayChoice::Auto)).unwrap();
        let mut running = Running::new(engine, pace);
        running.settle(false, Halt::Ask);
        running
    }

    fn held_back(dwell: Dwell) -> Running {
        let mut running = idle_with_a_spell(dwell);
        assert!(running.settle(false, Halt::Dwell).is_none());
        running
    }

    #[test]
    fn a_seat_that_cannot_act_in_its_own_main_phase_with_an_empty_stack_is_passed_for_at_once() {
        // Seat 0's first main phase, nothing in either hand: no dwell, though one is set.
        let pace = Pace {
            dwell: Dwell::fixed(500),
            ..Pace::MANUAL
        };
        let setup = Scenario::new(2).setup().unwrap();
        let mut running = Running::new(Engine::new(setup).unwrap(), pace);
        running.settle(false, Halt::Ask);
        assert_eq!(
            running.verdict_pending(SEATS[0], &Stops::new(2), &Yields::new(2)),
            Some(Verdict::Pass)
        );
        let mut events = Vec::new();
        let halt = running
            .skip_idle(&mut events, &mut Stops::new(2), &mut Yields::new(2))
            .unwrap();
        assert_eq!(halt, Halt::Ask, "nothing waits");
        assert!(!events.is_empty(), "seat 0 was passed for");
        assert_eq!(
            running.engine.decision().map(|d| d.player),
            Some(SEATS[1]),
            "until seat 1 draws a land it can play"
        );
    }

    #[test]
    fn the_run_stops_at_a_window_a_seat_cannot_use_and_passes_nothing_for_it() {
        let mut running = idle_with_a_spell(Dwell::fixed(500));
        let mut events = Vec::new();
        let halt = running
            .skip_idle(&mut events, &mut Stops::new(2), &mut Yields::new(2))
            .unwrap();
        assert_eq!(halt, Halt::Dwell);
        assert_eq!(events, [], "the pass waits for the dwell");
        assert_eq!(running.engine.decision().map(|d| d.player), Some(SEATS[0]));
    }

    #[test]
    fn with_the_dwell_off_the_run_passes_at_once_and_stops_where_a_seat_must_decide() {
        let mut running = idle_with_a_spell(Dwell::OFF);
        let mut events = Vec::new();
        let halt = running
            .skip_idle(&mut events, &mut Stops::new(2), &mut Yields::new(2))
            .unwrap();
        assert_eq!(halt, Halt::Ask);
        assert!(!events.is_empty(), "seat 0's idle windows were passed for");
        assert_eq!(
            running.engine.decision().map(|d| d.player),
            Some(SEATS[1]),
            "until seat 1 draws a land it can play"
        );
    }

    #[tokio::test(start_paused = true)]
    async fn a_decision_held_back_for_its_dwell_is_not_sent_answered_or_previewed() {
        let mut running = held_back(Dwell::fixed(500));
        let held = running.waiting_on();
        assert_eq!(held, Some(SEATS[0]));
        assert_eq!(
            running.due(),
            Some(Instant::now() + Duration::from_millis(500)),
            "the dwell is its clock"
        );

        let messages = running.sync_messages(SEATS[0]);
        assert!(
            matches!(messages.as_slice(), [ServerMessage::Snapshot { .. }]),
            "a snapshot taken meanwhile does not hand the seat the decision: {messages:?}"
        );
        // Whatever id a client guessed, the decision was never sent to it.
        for guess in 1..=3 {
            let id = DecisionId(guess);
            let refused = running.answer(SEATS[0], id, pace::pass()).unwrap_err();
            assert_eq!(refused.code, ErrorCode::StaleDecision);
            assert!(
                running
                    .preview(SEATS[0], id, &Answer::DeclareBlockers(Vec::new()))
                    .is_none()
            );
        }
        assert!(running.asks_priority_of(SEATS[0]));
    }

    #[tokio::test(start_paused = true)]
    async fn the_table_passes_for_a_held_back_seat_as_a_step_of_its_own() {
        let mut running = held_back(Dwell::fixed(500));
        let before = running.engine.events().len();
        let step = running.fall_back().unwrap();
        assert!(step.passed);
        assert_eq!(step.origin, SEATS[0]);
        assert_eq!(step.from_seq, count(before));
        assert!(matches!(
            step.events.first(),
            Some(Event::PriorityPassed { player }) if *player == SEATS[0]
        ));
    }

    #[tokio::test(start_paused = true)]
    async fn a_clock_belongs_to_a_decision_that_is_sent_and_a_dwell_to_one_that_is_held_back() {
        let pace = Pace {
            response: Some(Duration::from_secs(10)),
            ..Pace::MANUAL
        };
        let setup = Scenario::new(2).setup().unwrap();
        let mut running = Running::new(Engine::new(setup).unwrap(), pace);
        // Seat 0 is the active player, so it has no clock; the table's pass for it is the seat's own move.
        let first = running.settle(false, Halt::Ask).unwrap();
        assert_eq!(first.deadline_ms, None);
        let step = running.fall_back().unwrap();
        assert!(step.passed);
        // Seat 1 is not, so a window it is sent has the response clock, and the pass at its end is the same step.
        let afterwards = running.settle(false, Halt::Ask).unwrap();
        assert_eq!(afterwards.decision.player, SEATS[1]);
        assert_eq!(afterwards.deadline_ms, Some(10_000));
        let step = running.fall_back().unwrap();
        assert!(step.passed);
    }

    #[tokio::test(start_paused = true)]
    async fn a_kept_dwell_keeps_its_id_and_time_and_a_new_one_draws_again() {
        let mut running = held_back(Dwell::DEFAULT);
        let (id, due) = (running.pending.as_ref().unwrap().id, running.due());
        tokio::time::advance(Duration::from_millis(100)).await;
        assert!(running.settle(true, Halt::Dwell).is_none());
        assert_eq!(running.pending.as_ref().unwrap().id, id);
        assert_eq!(
            running.due(),
            due,
            "a concession elsewhere does not move it"
        );

        assert!(running.settle(false, Halt::Dwell).is_none());
        assert_ne!(running.pending.as_ref().unwrap().id, id);
        let wait = running.due().unwrap() - Instant::now();
        let (shortest, longest) = Dwell::DEFAULT.range();
        assert!((shortest..=longest).contains(&wait), "{wait:?}");
    }

    #[tokio::test(start_paused = true)]
    async fn when_the_rules_ask_a_held_back_seat_its_decision_is_issued_with_a_new_id() {
        let mut running = held_back(Dwell::fixed(500));
        let held = running.pending.as_ref().unwrap().id;
        let issued = running.settle(true, Halt::Ask).unwrap();
        assert_ne!(issued.id, held);
        assert_eq!(issued.decision.player, SEATS[0]);
        let messages = running.sync_messages(SEATS[0]);
        assert!(matches!(
            messages.as_slice(),
            [ServerMessage::Snapshot { .. }, ServerMessage::Decision { id, .. }] if *id == issued.id
        ));
        assert!(running.answer(SEATS[0], issued.id, pace::pass()).is_ok());
    }

    #[test]
    fn a_decision_is_held_back_only_while_the_rules_would_pass_for_its_seat() {
        let running = held_back(Dwell::fixed(500));
        let mut stops = Stops::new(2);
        let yields = Yields::new(2);
        let seat = SEATS[0];
        assert_eq!(
            running.verdict_pending(seat, &stops, &yields),
            Some(Verdict::Dwell)
        );
        assert!(running.passes_pending(seat, &stops, &yields));
        assert!(!running.dwell_ends(seat, &stops, &yields));
        assert_eq!(running.verdict_pending(SEATS[1], &stops, &yields), None);

        stops.set_full_control(seat, FullControlMode::NextWindow);
        assert_eq!(
            running.verdict_pending(seat, &stops, &yields),
            Some(Verdict::Ask)
        );
        assert!(running.dwell_ends(seat, &stops, &yields));
        assert!(!running.dwell_ends(SEATS[1], &stops, &yields));

        let mut stopped = Stops::new(2);
        let here = mtg_protocol::Stop {
            side: mtg_protocol::StopSide::Own,
            step: mtg_engine::turn::Step::Main1,
            keep: false,
        };
        stopped.set(seat, here).unwrap();
        assert!(
            running.dwell_ends(seat, &stopped, &yields),
            "a stop here asks"
        );

        let mut yielding = Yields::new(2);
        yielding.start(seat, YieldUntil::EndOfTurn, running.engine.state());
        assert_eq!(
            running.verdict_pending(seat, &Stops::new(2), &yielding),
            Some(Verdict::Pass),
            "a yield hurries the seat and skips the dwell"
        );
    }

    #[test]
    fn a_switched_off_dwell_makes_every_idle_window_a_pass_at_once() {
        let running = idle_with_a_spell(Dwell::OFF);
        let verdict = running.verdict_pending(SEATS[0], &Stops::new(2), &Yields::new(2));
        assert_eq!(verdict, Some(Verdict::Pass));
    }

    #[tokio::test]
    async fn the_dwells_follow_neither_the_game_seed_nor_each_other() {
        // Two games of the same seed and cards draw their dwells apart: the draws come from the operating system.
        let draw = || {
            let mut running = idle_with_a_spell(Dwell::DEFAULT);
            (0..8).map(|_| running.waits.wait()).collect::<Vec<_>>()
        };
        assert_ne!(draw(), draw());
    }
}
