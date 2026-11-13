//! Time management for one decision, as chess engines budget a move: the decision's importance scales the search
//! (combat when lethal is in reach, a big opposing spell on the stack and the seat's big plays get most; a pass with
//! nothing to do gets none), a soft limit ends the search once its favourite is stable, and a hard limit ends it
//! whatever the state, always within [`DECISION_CAP`] of the decision's start.

use std::time::{Duration, Instant};

use mtg_engine::Decision;
use mtg_engine::decision::DecisionKind;

use super::eval::pressure;
use super::priority::{self, Window};
use super::seen::Seen;
use super::threat;

/// The longest one decision may take, all searches included: under the server's 5 s think deadline.
pub const DECISION_CAP: Duration = Duration::from_millis(4_500);

/// The most an important decision multiplies its level's budget by.
pub const MOST: f64 = 3.0;

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
        let cap = started + DECISION_CAP - SLACK;
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
        self.hard.is_some_and(|hard| Instant::now() >= hard)
    }

    /// Whether the soft limit has passed.
    pub fn past_soft(&self) -> bool {
        self.soft.is_some_and(|soft| Instant::now() >= soft)
    }
}

#[cfg(test)]
mod tests {
    //! Goal: a decision's budget scales with its importance and never reaches past the decision cap.
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
    fn the_hard_limit_stays_within_the_cap(#[values(0.5, 1.0, 2.0)] importance: f64) {
        let started = Instant::now();
        let budget = Budget::new(started, 600, Some(Duration::from_secs(4)), importance, 1.0);
        let (soft, hard) = (budget.soft.unwrap(), budget.hard.unwrap());
        assert!(soft <= hard);
        assert!(hard <= started + DECISION_CAP);
    }
}
