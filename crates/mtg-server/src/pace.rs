//! Pace of play: passing for a seat the windows pass for or that yields, and the clocks that keep one player from
//! holding the table.
//!
//! All of them work through the same path as a seat's own answer ([`crate::running::Running::answer`]), so what a
//! pass or a timeout does to the log, the store and every projection is exactly what the seat's click would have done.
//! When a seat is asked is decided in [`crate::windows`] and when a yield stops in [`crate::yields`], alone. A seat that
//! cannot act at a window where it waits is passed for the same way once its dwell ([`crate::dwell`]) is over.

use std::time::Duration;

use mtg_core::ids::PlayerId;
use mtg_engine::decision::PriorityAction;
use mtg_engine::{Answer, Decision, DecisionKind};
use mtg_protocol::{ErrorCode, FullControlMode, ServerMessage, YieldUntil};
use tokio::time::Instant;

use crate::actor::{Fatal, GameActor, Phase, not_started};
use crate::dwell::Dwell;
use crate::running::{Halt, Rejection};
use crate::store::EventStore;
use crate::windows::Verdict;
use crate::yields::Yields;

/// Longest clock the settings accept, in seconds (a day), so a deadline can never overflow.
pub const MAX_CLOCK_SECS: u64 = 86_400;

/// How long after a failed save the server tries again to answer for a seat whose clock ran out.
const RETRY_AFTER: Duration = Duration::from_secs(1);

/// How a game moves: how seats start out (auto-pass on, or in Full Control), how long a seat may take to respond and how
/// long the table waits before it passes for a seat that cannot act.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Pace {
    /// Whether each seat starts with auto-pass on (`MTG_AUTO_PASS`): in Full Control mode `Off`, or `Held` when off.
    /// A seat can change its own with `AutoPass` and `FullControl`.
    pub auto_pass: bool,
    /// Time a seat that is not the active player has to answer a priority decision, whether it can act or not
    /// (`MTG_RESPONSE_SECS`); `None` is no clock.
    pub response: Option<Duration>,
    /// Time a defending seat has to declare blockers (`MTG_BLOCK_SECS`); `None` is no clock.
    pub blocks: Option<Duration>,
    /// How long the table waits before it passes for an idle seat at a window of the default table, its own main phase
    /// with an empty stack aside (`MTG_DWELL_MS`).
    pub dwell: Dwell,
}

impl Pace {
    /// No auto-pass and no clocks: every decision waits for its player, as in a game played straight on an engine.
    pub const MANUAL: Pace = Pace {
        auto_pass: false,
        response: None,
        blocks: None,
        dwell: Dwell::OFF,
    };

    /// The Full Control mode every seat starts in: `Off` with auto-pass on, `Held` with it off.
    pub(crate) const fn mode(&self) -> FullControlMode {
        if self.auto_pass {
            FullControlMode::Off
        } else {
            FullControlMode::Held
        }
    }

    /// The clock `decision` runs on, if any.
    ///
    /// Only a seat that is not the active player is hurried, and only for a priority decision or for
    /// declaring blockers; every other decision (mulligans, discards, damage division, the active
    /// player's own turn) waits for as long as it takes. A priority window the seat has nothing to do in
    /// is hurried too (it is only delivered at a stop or in Full Control): it would hold the table for no reason.
    pub(crate) fn clock_for(&self, decision: &Decision, active: PlayerId) -> Option<Duration> {
        if decision.player == active {
            return None;
        }
        match &decision.kind {
            DecisionKind::Priority(_) => self.response,
            DecisionKind::DeclareBlockers(_) => self.blocks,
            _ => None,
        }
    }
}

impl Default for Pace {
    fn default() -> Self {
        Pace {
            auto_pass: true,
            response: Some(Duration::from_secs(10)),
            blocks: Some(Duration::from_secs(45)),
            dwell: Dwell::DEFAULT,
        }
    }
}

/// A priority pass.
pub(crate) fn pass() -> Answer {
    Answer::Priority(PriorityAction::Pass)
}

/// Whether the decision is a priority decision, whatever its options.
pub(crate) fn is_priority(decision: &Decision) -> bool {
    matches!(&decision.kind, DecisionKind::Priority(_))
}

/// What the table answers for a seat whose clock ran out: a pass, or no blockers.
///
/// `None` for a decision that has no clock and no safe default.
pub(crate) fn fallback(decision: &Decision) -> Option<Answer> {
    match &decision.kind {
        DecisionKind::Priority(_) => Some(pass()),
        DecisionKind::DeclareBlockers(_) => Some(Answer::DeclareBlockers(Vec::new())),
        _ => None,
    }
}

impl<S: EventStore> GameActor<S> {
    /// When the pending decision's clock runs out, if it has one.
    pub(crate) fn due(&self) -> Option<Instant> {
        match &self.phase {
            Phase::Running(running) => running.due(),
            Phase::Lobby { .. } => None,
        }
    }

    /// Answers for the seat whose clock ran out, with a pass or no blocks, as if the seat had. The same step passes for a
    /// seat that cannot act once its dwell is over: the decision was never sent, and the pass is saved, projected and
    /// sent like the seat's own, as a batch of its own.
    pub(crate) async fn expire(&mut self) -> Result<(), Fatal> {
        let Phase::Running(running) = &mut self.phase else {
            return Ok(());
        };
        if let Ok(step) = running.fall_back() {
            // If the move cannot be saved the decision stays; trying again at once would spin.
            running.postpone(Instant::now() + RETRY_AFTER);
            self.publish(step).await
        } else {
            running.stop_clock();
            Ok(())
        }
    }

    /// Starts `seat`'s yield; if the seat faces a priority decision it is passed at once.
    ///
    /// The yield, the passes it makes and their save are one unit, like an answer and the auto-passes after it: a
    /// failed save keeps no yield. A yield that ends at once (the stack it waits on is empty, or the seat holds a
    /// stop here) passes nothing and the decision is sent again with a new id, so the seat is never left without the
    /// prompt it owes an answer to. A seat in Full Control cannot yield: it is asked at every window.
    pub(crate) async fn set_yield(
        &mut self,
        seat: PlayerId,
        until: YieldUntil,
    ) -> Result<(), Fatal> {
        let Phase::Running(running) = &mut self.phase else {
            self.refuse(seat, not_started());
            return Ok(());
        };
        let state = running.engine.state();
        if running.engine.result().is_some() || !state.is_alive(seat) {
            let reason = "you are not in a game that is still being played";
            self.refuse(seat, Rejection::new(ErrorCode::BadMessage, reason));
            return Ok(());
        }
        if self.stops.full_control(seat) != FullControlMode::Off {
            let reason =
                "full control is on, so the server cannot pass for you; turn it off to yield";
            self.refuse(seat, Rejection::new(ErrorCode::BadMessage, reason));
            return Ok(());
        }
        let mut yields = self.yields.clone();
        if !running.asks_priority_of(seat) {
            yields.start(seat, until, state);
            self.commit_yields(yields);
            return Ok(());
        }
        // The seat asks while it looks at a window, which a stop of its own there does not take back.
        yields.start_facing(seat, until, state);
        match running.restart_decision(seat) {
            Ok(step) => {
                let stops = self.stops.clone();
                self.publish_with(step, yields, stops).await
            }
            Err(rejection) => {
                self.refuse(seat, rejection);
                Ok(())
            }
        }
    }

    /// Ends `seat`'s yield; the seat is told if it had one.
    pub(crate) fn cancel_yield(&mut self, seat: PlayerId) {
        let mut yields = self.yields.clone();
        if yields.cancel(seat) {
            self.commit_yields(yields);
        }
    }

    /// Takes `yields` as the seats' yields and tells each seat whose yield changed.
    pub(crate) fn commit_yields(&mut self, yields: Yields) {
        for (seat, until) in self.yields.replace(yields) {
            self.seats.send(seat, ServerMessage::Yielding { until });
        }
    }

    /// Settles what a game that is starting or resumed from its log left pending: if the windows pass for the seat
    /// it waits on, it is passed for before any client connects. A seat that must wait at its window gets a dwell that
    /// starts now, like one that meets such a window while the game runs.
    pub(crate) async fn start_up(&mut self) -> Result<(), Fatal> {
        let Phase::Running(running) = &mut self.phase else {
            return Ok(());
        };
        let Some(seat) = running.waiting_on() else {
            return Ok(());
        };
        match running.verdict_pending(seat, &self.stops, &self.yields) {
            Some(Verdict::Pass) => match running.fall_back() {
                Ok(step) => self.publish(step).await,
                Err(rejection) => {
                    self.refuse(seat, rejection);
                    Ok(())
                }
            },
            Some(Verdict::Dwell) => {
                running.settle(false, Halt::Dwell, false);
                Ok(())
            }
            Some(Verdict::Ask) | None => Ok(()),
        }
    }
}

#[cfg(test)]
mod tests {
    //! Goal: the pace rules give the clock, the safe default and the priority test exactly to the decisions they are meant for, whatever the settings, and a seat starts in the mode its auto-pass default names.
    use super::*;
    use mtg_core::ids::ObjectId;
    use mtg_engine::decision::{
        AttackOptions, BlockOptions, BottomOptions, DamageOptions, DiscardOptions, MulliganOptions,
        PayOptions, PriorityOptions,
    };
    use mtg_engine::mana::ManaPool;
    use rstest::rstest;

    const ACTIVE: PlayerId = PlayerId(0);
    const OTHER: PlayerId = PlayerId(1);

    fn priority(can_act: bool) -> DecisionKind {
        DecisionKind::Priority(PriorityOptions {
            playable_lands: Vec::new(),
            castable: Vec::new(),
            mana_abilities: Vec::new(),
            abilities: Vec::new(),
            can_act,
            can_pass: true,
        })
    }

    fn blockers() -> DecisionKind {
        DecisionKind::DeclareBlockers(BlockOptions {
            blockers: Vec::new(),
        })
    }

    fn other_kinds() -> Vec<DecisionKind> {
        vec![
            DecisionKind::Mulligan(MulliganOptions {
                hand: Vec::new(),
                mulligans_taken: 0,
                can_mulligan: true,
                free_mulligans: 0,
                bottom_if_keep: 0,
                bottom_if_mulligan: 1,
            }),
            DecisionKind::BottomCards(BottomOptions {
                hand: Vec::new(),
                count: 1,
            }),
            DecisionKind::PayMana(PayOptions {
                spell: ObjectId(1),
                cost: mtg_core::mana::ManaCost::parse("{1}").unwrap(),
                pool: ManaPool::default(),
                sources: Vec::new(),
                auto_available: true,
                can_cancel: true,
                life: 20,
                max_life: 0,
            }),
            DecisionKind::DeclareAttackers(AttackOptions {
                attackers: Vec::new(),
            }),
            DecisionKind::AssignDamage(DamageOptions {
                attacker: ObjectId(1),
                power: 2,
                blockers: Vec::new(),
            }),
            DecisionKind::Discard(DiscardOptions {
                hand: Vec::new(),
                count: 1,
            }),
        ]
    }

    fn decision(player: PlayerId, kind: DecisionKind) -> Decision {
        Decision { player, kind }
    }

    fn pace() -> Pace {
        Pace {
            auto_pass: true,
            response: Some(Duration::from_secs(10)),
            blocks: Some(Duration::from_secs(45)),
            dwell: Dwell::OFF,
        }
    }

    #[rstest]
    #[case::response_to_an_opponents_turn(OTHER, priority(true), Some(10))]
    #[case::nothing_to_respond_with_is_hurried_too(OTHER, priority(false), Some(10))]
    #[case::the_active_players_own_priority(ACTIVE, priority(true), None)]
    #[case::the_active_player_with_nothing_to_do(ACTIVE, priority(false), None)]
    #[case::a_defender_blocking(OTHER, blockers(), Some(45))]
    fn clocks_by_decision(
        #[case] player: PlayerId,
        #[case] kind: DecisionKind,
        #[case] seconds: Option<u64>,
    ) {
        let got = pace().clock_for(&decision(player, kind), ACTIVE);
        assert_eq!(got, seconds.map(Duration::from_secs));
    }

    #[test]
    fn every_other_decision_waits_as_long_as_it_takes() {
        for kind in other_kinds() {
            for player in [ACTIVE, OTHER] {
                let d = decision(player, kind.clone());
                assert_eq!(pace().clock_for(&d, ACTIVE), None, "{d:?}");
            }
        }
    }

    #[rstest]
    #[case::manual(Pace::MANUAL)]
    #[case::response_off(Pace { response: None, ..pace() })]
    fn a_missing_setting_means_no_clock(#[case] pace: Pace) {
        assert_eq!(
            pace.clock_for(&decision(OTHER, priority(true)), ACTIVE),
            None
        );
        if pace.blocks.is_none() {
            assert_eq!(pace.clock_for(&decision(OTHER, blockers()), ACTIVE), None);
        }
    }

    #[test]
    fn the_defaults_are_ten_and_forty_five_seconds_and_a_dwell_with_auto_pass_on() {
        let pace = Pace::default();
        assert!(pace.auto_pass);
        assert_eq!(pace.response, Some(Duration::from_secs(10)));
        assert_eq!(pace.blocks, Some(Duration::from_secs(45)));
        assert_eq!(
            pace.dwell.range(),
            (Duration::from_millis(300), Duration::from_millis(900))
        );
        assert_eq!(Pace::MANUAL.response, None);
        assert!(!std::hint::black_box(Pace::MANUAL).auto_pass);
        assert!(Pace::MANUAL.dwell.is_off(), "the manual pace never waits");
    }

    #[rstest]
    #[case::pass_for_a_priority_holder(priority(true), Some(pass()))]
    #[case::pass_even_when_idle(priority(false), Some(pass()))]
    #[case::no_blocks(blockers(), Some(Answer::DeclareBlockers(Vec::new())))]
    fn safe_defaults(#[case] kind: DecisionKind, #[case] expected: Option<Answer>) {
        assert_eq!(fallback(&decision(OTHER, kind)), expected);
    }

    #[test]
    fn nothing_else_has_a_safe_default() {
        for kind in other_kinds() {
            assert_eq!(fallback(&decision(OTHER, kind)), None);
        }
    }

    #[rstest]
    #[case::auto_pass_on(true, FullControlMode::Off)]
    #[case::auto_pass_off(false, FullControlMode::Held)]
    fn a_seat_starts_in_the_mode_the_auto_pass_default_names(
        #[case] auto_pass: bool,
        #[case] mode: FullControlMode,
    ) {
        assert_eq!(
            Pace {
                auto_pass,
                ..pace()
            }
            .mode(),
            mode
        );
    }

    #[test]
    fn only_priority_is_a_priority_decision() {
        assert!(is_priority(&decision(OTHER, priority(true))));
        assert!(is_priority(&decision(OTHER, priority(false))));
        assert!(!is_priority(&decision(OTHER, blockers())));
        for kind in other_kinds() {
            assert!(!is_priority(&decision(OTHER, kind)));
        }
    }
}
