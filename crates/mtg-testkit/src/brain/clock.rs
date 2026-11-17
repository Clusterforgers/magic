//! Time management for one decision, as chess engines budget a move: the decision's importance scales the search
//! (combat when lethal is in reach, a big opposing spell on the stack and the seat's big plays get most; a pass with
//! nothing to do gets none), a soft limit ends the search once its favourite is stable, and a hard limit ends it
//! whatever the state, always within [`DECISION_CAP`] of the decision's start. A [`Stop`] set around a decision
//! ([`stopping`]) is the hard stop every loop of the search checks ([`stopped`]): its time, or its flag raised by the
//! caller that gave up waiting.

use std::cell::RefCell;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use mtg_engine::Decision;
use mtg_engine::decision::DecisionKind;

use super::eval::pressure;
use super::priority::{self, Window};
use super::seen::Seen;
use super::threat;

/// The longest one decision may take, all searches included: under the server's 5 s think deadline.
pub const DECISION_CAP: Duration = Duration::from_millis(4_500);

/// When the decisions answered inside [`stopping`] stop searching: at a time, or once a flag is raised.
#[derive(Debug, Clone, Default)]
pub struct Stop {
    at: Option<Instant>,
    cancel: Option<Arc<AtomicBool>>,
}

impl Stop {
    /// A stop at `at`.
    pub fn at(at: Instant) -> Stop {
        Stop {
            at: Some(at),
            cancel: None,
        }
    }

    /// This stop that also comes once `cancel` is raised.
    #[must_use]
    pub fn or_cancel(mut self, cancel: Arc<AtomicBool>) -> Stop {
        self.cancel = Some(cancel);
        self
    }

    /// Whether the stop has come.
    pub fn reached(&self) -> bool {
        self.cancel
            .as_ref()
            .is_some_and(|cancel| cancel.load(Ordering::Relaxed))
            || self.at.is_some_and(|at| Instant::now() >= at)
    }

    /// The earlier of this stop and `outer`, either's flag stopping it.
    fn within(self, outer: Option<&Stop>) -> Stop {
        let Some(outer) = outer else {
            return self;
        };
        Stop {
            at: match (self.at, outer.at) {
                (Some(a), Some(b)) => Some(a.min(b)),
                (a, b) => a.or(b),
            },
            cancel: self.cancel.or_else(|| outer.cancel.clone()),
        }
    }
}

thread_local! {
    static STOP: RefCell<Option<Stop>> = const { RefCell::new(None) };
}

/// Runs `work` with `stop` as the hard stop of every search on this thread (the earlier of it and one already set).
pub fn stopping<T>(stop: Stop, work: impl FnOnce() -> T) -> T {
    /// Puts the stop that was set before back, also when `work` panics.
    struct Restore(Option<Stop>);
    impl Drop for Restore {
        fn drop(&mut self) {
            let before = self.0.take();
            STOP.with(|slot| *slot.borrow_mut() = before);
        }
    }
    let before = STOP.with(|slot| slot.borrow().clone());
    let inner = stop.within(before.as_ref());
    STOP.with(|slot| *slot.borrow_mut() = Some(inner));
    let _restore = Restore(before);
    work()
}

/// Whether the hard stop of the decision being answered on this thread has come; never without a [`stopping`] around.
pub fn stopped() -> bool {
    STOP.with(|slot| slot.borrow().as_ref().is_some_and(Stop::reached))
}

/// The most an important decision multiplies its level's budget by.
pub const MOST: f64 = 3.0;

/// Permanents on the battlefield up to which a search gets its whole budget.
const FULL_BOARD: usize = 30;

/// The least share of its budget a search on a crowded board gets.
const LEAST_BOARD_SHARE: f64 = 0.35;

/// The share of its budget a search gets on a battlefield of `permanents`: all of it up to [`FULL_BOARD`], less in
/// proportion beyond, never under [`LEAST_BOARD_SHARE`]. Every simulated step costs more on a crowded board, so the
/// search ends sooner and each loop's last step overshoots its stop less.
pub fn board_share(permanents: usize) -> f64 {
    if permanents <= FULL_BOARD {
        return 1.0;
    }
    let ratio = f64::from(u32::try_from(FULL_BOARD).unwrap_or(u32::MAX))
        / f64::from(u32::try_from(permanents).unwrap_or(u32::MAX));
    ratio.max(LEAST_BOARD_SHARE)
}

/// The time a search leaves before the cap for the answer to come back (a rollout step finishing, the reply).
const SLACK: Duration = Duration::from_millis(300);

/// How much of its level's search budget `decision` deserves, from 0 (nothing to think about) to [`MOST`].
pub fn importance(seen: &Seen<'_>, decision: &Decision) -> f64 {
    let lethal = lethal_in_reach(seen);
    match &decision.kind {
        DecisionKind::DeclareAttackers(_) | DecisionKind::DeclareBlockers(_) => {
            if lethal {
                MOST
            } else {
                1.5
            }
        }
        DecisionKind::Priority(options) => match priority::window(seen) {
            Window::Idle => 0.0,
            Window::Response => {
                let threat = seen
                    .stack()
                    .last()
                    .map_or(0.0, |top| threat::spell_threat(seen, *top));
                if threat >= threat::hold_value(seen) || lethal {
                    2.5
                } else {
                    0.6
                }
            }
            Window::Main => {
                let big = options.castable.iter().any(|id| seen.mana_value(*id) >= 4);
                if lethal {
                    MOST
                } else if big {
                    1.5
                } else {
                    0.6
                }
            }
            Window::Combat => {
                if lethal {
                    2.0
                } else {
                    1.0
                }
            }
            Window::EndStep => 0.5,
        },
        // A triggered ability's targets on a crowded board are many and each is simulated: an ordinary share keeps the
        // answer well within the server's think deadline.
        DecisionKind::ChooseTargets(options) if options.triggered => 0.5,
        DecisionKind::ChooseTargets(_) => 1.0,
        _ => 0.5,
    }
}

/// Whether some team's creatures threaten lethal damage against another's on their next attack.
pub fn lethal_in_reach(seen: &Seen<'_>) -> bool {
    let bodies = seen.bodies();
    let alive = seen.alive();
    alive.iter().any(|attacker| {
        alive.iter().any(|defender| {
            !seen.same_team(*attacker, *defender)
                && pressure(
                    seen,
                    &bodies,
                    seen.team_of(*attacker),
                    seen.team_of(*defender),
                ) >= seen.life(*defender)
        })
    })
}

/// When a search of a decision that started at `started` stops at the latest: [`DECISION_CAP`] less the slack the
/// answer needs to come back.
pub fn cap(started: Instant) -> Instant {
    started + DECISION_CAP.saturating_sub(SLACK)
}

/// The search budget of one decision.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Budget {
    /// The most iterations.
    pub iterations: u32,
    /// After this the search stops once its favourite is stable.
    pub soft: Option<Instant>,
    /// The search stops here whatever its state.
    pub hard: Option<Instant>,
}

impl Budget {
    /// The budget of a decision that started at `started`, worth `importance`, for a search of `iterations` and
    /// `time` (none when untimed), at `share` of it (a busy server thinks less).
    pub fn new(
        started: Instant,
        iterations: u32,
        time: Option<Duration>,
        importance: f64,
        share: f64,
    ) -> Budget {
        let scale = (importance * share.clamp(0.0, 1.0)).clamp(0.0, MOST);
        // In range: at most MOST times the level's iterations.
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let iterations = (f64::from(iterations) * scale).round() as u32;
        let cap = cap(started);
        let soft = time.map(|time| (Instant::now() + time.mul_f64(scale)).min(cap));
        let hard = time.map(|time| (Instant::now() + time.mul_f64(scale * 1.5)).min(cap));
        Budget {
            iterations,
            soft,
            hard,
        }
    }

    /// Whether the hard limit has passed.
    pub fn out(&self) -> bool {
        self.hard.is_some_and(|hard| Instant::now() >= hard) || stopped()
    }

    /// Whether the soft limit has passed.
    pub fn past_soft(&self) -> bool {
        self.soft.is_some_and(|soft| Instant::now() >= soft)
    }
}

#[cfg(test)]
mod tests {
    //! Goal: a decision's budget scales with its importance and the board's size and never reaches past the decision cap,
    //! and a stop ends it at its time or its flag, the earlier of nested stops holding.
    use rstest::rstest;

    use super::*;

    #[rstest]
    #[case(0.0, 0)]
    #[case(0.5, 300)]
    #[case(1.0, 600)]
    #[case(5.0, 1800)]
    fn iterations_follow_importance(#[case] importance: f64, #[case] expected: u32) {
        let budget = Budget::new(Instant::now(), 600, None, importance, 1.0);
        assert_eq!(budget.iterations, expected);
        assert!(budget.soft.is_none() && budget.hard.is_none() && !budget.out());
    }

    #[rstest]
    #[case::none_set(None, false, false)]
    #[case::still_ahead(Some(60_000), false, false)]
    #[case::passed(Some(0), false, true)]
    #[case::cancelled(Some(60_000), true, true)]
    #[case::cancelled_without_a_time(None, true, true)]
    fn a_stop_comes_at_its_time_or_its_flag(
        #[case] in_ms: Option<u64>,
        #[case] raised: bool,
        #[case] expected: bool,
    ) {
        let cancel = Arc::new(AtomicBool::new(raised));
        let stop = match in_ms {
            Some(ms) => Stop::at(Instant::now() + Duration::from_millis(ms)),
            None => Stop::default(),
        }
        .or_cancel(cancel);
        assert!(!stopped(), "no stop outside");
        assert_eq!(stopping(stop, stopped), expected);
        assert!(!stopped(), "the stop ends with its work");
    }

    #[rstest]
    fn the_earlier_of_two_nested_stops_holds_and_the_outer_one_returns(
        #[values(true, false)] inner_first: bool,
    ) {
        let now = Instant::now();
        let (soon, late) = (now, now + Duration::from_secs(60));
        let (outer, inner) = if inner_first {
            (late, soon)
        } else {
            (soon, late)
        };
        let (inside, after) = stopping(Stop::at(outer), || {
            (stopping(Stop::at(inner), stopped), stopped())
        });
        assert!(inside, "the earlier stop has come inside");
        assert_eq!(
            after, !inner_first,
            "the outer stop alone holds after the inner one"
        );
    }

    #[test]
    fn the_hard_limit_ends_at_a_stop() {
        let budget = Budget::new(Instant::now(), 600, Some(Duration::from_secs(4)), 1.0, 1.0);
        assert!(!budget.out());
        let cancel = Arc::new(AtomicBool::new(true));
        assert!(stopping(Stop::default().or_cancel(cancel), || budget.out()));
    }

    #[rstest]
    fn a_crowded_board_gets_a_smaller_share(
        #[values(0, 10, 30, 31, 45, 60, 90, 200, 10_000)] permanents: usize,
    ) {
        let share = board_share(permanents);
        assert!((LEAST_BOARD_SHARE..=1.0).contains(&share));
        assert!(board_share(permanents + 1) <= share);
        assert_eq!(share >= 1.0, permanents <= FULL_BOARD);
    }

    #[rstest]
    fn the_hard_limit_stays_within_the_cap(#[values(0.5, 1.0, 2.0)] importance: f64) {
        let started = Instant::now();
        let budget = Budget::new(started, 600, Some(Duration::from_secs(4)), importance, 1.0);
        let (soft, hard) = (budget.soft.unwrap(), budget.hard.unwrap());
        assert!(soft <= hard);
        assert!(hard <= started + DECISION_CAP);
    }
}
