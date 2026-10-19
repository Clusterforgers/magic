//! Pace of play: passing for a seat that has nothing to do or that yields, and the clocks that keep one player from
//! holding the table.
//!
//! All of them work through the same path as a seat's own answer ([`crate::running::Running::answer`]), so what a
//! pass or a timeout does to the log, the store and every projection is exactly what the seat's click would have done.
//! When a yield stops is decided in [`crate::yields`] alone.

use std::time::Duration;

use mtg_core::ids::PlayerId;
use mtg_engine::decision::PriorityAction;
use mtg_engine::{Answer, Decision, DecisionKind, GameState};
use mtg_protocol::{ErrorCode, ServerMessage, YieldUntil};
use tokio::time::Instant;

use crate::actor::{Fatal, GameActor, Phase, not_started};
use crate::running::Rejection;
use crate::store::EventStore;
use crate::yields::Yields;

/// Longest clock the settings accept, in seconds (a day), so a deadline can never overflow.
pub const MAX_CLOCK_SECS: u64 = 86_400;

/// How long after a failed save the server tries again to answer for a seat whose clock ran out.
const RETRY_AFTER: Duration = Duration::from_secs(1);

/// How a game moves: whether a seat with nothing to do is passed for, and how long a seat may take to respond.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Pace {
    /// Whether each seat starts with auto-pass on (`MTG_AUTO_PASS`); a seat can change its own with `AutoPass`.
    pub auto_pass: bool,
    /// Time a seat that is not the active player has to answer a priority decision, whether it can act or not
    /// (`MTG_RESPONSE_SECS`); `None` is no clock.
    pub response: Option<Duration>,
    /// Time a defending seat has to declare blockers (`MTG_BLOCK_SECS`); `None` is no clock.
    pub blocks: Option<Duration>,
}

impl Pace {
    /// No auto-pass and no clocks: every decision waits for its player, as in a game played straight on an engine.
    pub const MANUAL: Pace = Pace {
        auto_pass: false,
        response: None,
        blocks: None,
    };

    /// The clock `decision` runs on, if any.
    ///
    /// Only a seat that is not the active player is hurried, and only for a priority decision or for
    /// declaring blockers; every other decision (mulligans, discards, damage division, the active
    /// player's own turn) waits for as long as it takes. A priority window the seat has nothing to do in
    /// is hurried too (it is only delivered with auto-pass off): it would hold the table for no reason.
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
        }
    }
}

/// A priority pass.
pub(crate) fn pass() -> Answer {
    Answer::Priority(PriorityAction::Pass)
}

/// A priority decision whose holder can do nothing useful: only tapping mana is left.
pub(crate) fn idle(decision: &Decision) -> bool {
    matches!(&decision.kind, DecisionKind::Priority(options) if !options.can_act)
}

/// Whether the decision is a priority decision, whatever its options.
pub(crate) fn is_priority(decision: &Decision) -> bool {
    matches!(&decision.kind, DecisionKind::Priority(_))
}

/// Whether the server passes for the decision's seat without asking: it is idle and the seat has auto-pass on.
pub(crate) fn skippable(decision: &Decision, auto_pass: &[bool]) -> bool {
    idle(decision)
        && auto_pass
            .get(decision.player.index())
            .copied()
            .unwrap_or(false)
}

/// Whether the server passes for the decision's seat without asking: the seat yields, or it is idle with
/// auto-pass on. A yield that this decision ends is dropped from `yields` and the auto-pass rule decides.
pub(crate) fn passes_for(
    decision: &Decision,
    auto_pass: &[bool],
    yields: &mut Yields,
    state: &GameState,
) -> bool {
    is_priority(decision)
        && (yields.passes(decision.player, state) || skippable(decision, auto_pass))
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

    /// Answers for the seat whose clock ran out, with a pass or no blocks, as if the seat had.
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

    /// Records the seat's auto-pass setting; turning it on passes at once if the seat is facing a decision it cannot use.
    pub(crate) async fn set_auto_pass(
        &mut self,
        seat: PlayerId,
        enabled: bool,
    ) -> Result<(), Fatal> {
        if let Some(flag) = self.auto_pass.get_mut(seat.index()) {
            *flag = enabled;
        }
        if enabled {
            self.pass_if_idle(seat).await
        } else {
            Ok(())
        }
    }

    /// Starts `seat`'s yield; if the seat faces a priority decision it is passed at once.
    ///
    /// The yield, the passes it makes and their save are one unit, like an answer and the auto-passes after it: a
    /// failed save keeps no yield. A yield that ends at once (the stack it waits on is empty) passes nothing and the
    /// decision is sent again with a new id, so the seat is never left without the prompt it owes an answer to.
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
        let mut yields = self.yields.clone();
        yields.start(seat, until, state);
        if !running.asks_priority_of(seat) {
            self.commit_yields(yields);
            return Ok(());
        }
        match running.restart_decision(seat) {
            Ok(step) => self.publish_with(step, yields).await,
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
    fn commit_yields(&mut self, yields: Yields) {
        for (seat, until) in self.yields.replace(yields) {
            self.seats.send(seat, ServerMessage::Yielding { until });
        }
    }

    /// Passes for `seat` if the decision pending for it cannot be used and the seat has auto-pass on.
    async fn pass_if_idle(&mut self, seat: PlayerId) -> Result<(), Fatal> {
        let Phase::Running(running) = &mut self.phase else {
            return Ok(());
        };
        if !running.idle_for(seat, &self.auto_pass) {
            return Ok(());
        }
        match running.fall_back() {
            Ok(step) => self.publish(step).await,
            Err(rejection) => {
                self.refuse(seat, rejection);
                Ok(())
            }
        }
    }

    /// Settles what a game resumed from its log left pending: a seat that can do nothing is passed for.
    pub(crate) async fn start_up(&mut self) -> Result<(), Fatal> {
        let waiting = match &self.phase {
            Phase::Running(running) => running.waiting_on(),
            Phase::Lobby { .. } => None,
        };
        match waiting {
            Some(seat) => self.pass_if_idle(seat).await,
            None => Ok(()),
        }
    }
}

#[cfg(test)]
mod tests {
    //! Goal: the pace rules give the clock, the safe default and the idle test exactly to the decisions they are meant for, whatever the settings.
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
    fn the_defaults_are_ten_and_forty_five_seconds_with_auto_pass_on() {
        let pace = Pace::default();
        assert!(pace.auto_pass);
        assert_eq!(pace.response, Some(Duration::from_secs(10)));
        assert_eq!(pace.blocks, Some(Duration::from_secs(45)));
        assert_eq!(Pace::MANUAL.response, None);
        assert!(!std::hint::black_box(Pace::MANUAL).auto_pass);
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
    #[case::idle_and_on(priority(false), &[true, true], true)]
    #[case::idle_but_off(priority(false), &[true, false], false)]
    #[case::can_act(priority(true), &[true, true], false)]
    #[case::blockers_are_never_skipped(blockers(), &[true, true], false)]
    #[case::unknown_seat_counts_as_off(priority(false), &[true], false)]
    fn skipping(#[case] kind: DecisionKind, #[case] flags: &[bool], #[case] skipped: bool) {
        assert_eq!(skippable(&decision(OTHER, kind), flags), skipped);
    }

    #[test]
    fn other_kinds_are_never_idle() {
        for kind in other_kinds() {
            assert!(!idle(&decision(OTHER, kind)));
        }
    }
}
