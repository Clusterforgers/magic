//! Pace of play: passing for a seat that has nothing to do.
//!
//! The passes go through the same path as a seat's own answer ([`crate::running::Running::answer`]), so
//! what a pass does to the log, the store and every projection is exactly what the seat's click would have done.

use mtg_core::ids::PlayerId;
use mtg_engine::decision::PriorityAction;
use mtg_engine::{Answer, Decision, DecisionKind};

use crate::actor::{Fatal, GameActor, Phase};
use crate::store::EventStore;

/// How a game moves: whether a seat with nothing to do is passed for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Pace {
    /// Whether each seat starts with auto-pass on (`MTG_AUTO_PASS`); a seat can change its own with `AutoPass`.
    pub auto_pass: bool,
}

impl Pace {
    /// No auto-pass: every decision waits for its player, as in a game played straight on an engine.
    pub const MANUAL: Pace = Pace { auto_pass: false };
}

impl Default for Pace {
    fn default() -> Self {
        Pace { auto_pass: true }
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

/// Whether the server passes for the decision's seat without asking: it is idle and the seat has auto-pass on.
pub(crate) fn skippable(decision: &Decision, auto_pass: &[bool]) -> bool {
    idle(decision)
        && auto_pass
            .get(decision.player.index())
            .copied()
            .unwrap_or(false)
}

impl<S: EventStore> GameActor<S> {
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

    /// Passes for `seat` if the decision pending for it cannot be used and the seat has auto-pass on.
    async fn pass_if_idle(&mut self, seat: PlayerId) -> Result<(), Fatal> {
        let Phase::Running(running) = &mut self.phase else {
            return Ok(());
        };
        if !running.idle_for(seat, &self.auto_pass) {
            return Ok(());
        }
        match running.pass_for_pending() {
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
    //! Goal: the idle test and the skip rule hold exactly for a priority decision nobody can use at a seat that asked to be passed for, and for no other kind of decision.
    use super::*;
    use mtg_core::ids::ObjectId;
    use mtg_engine::decision::{
        AttackOptions, BlockOptions, BottomOptions, DamageOptions, DiscardOptions, MulliganOptions,
        PayOptions, PriorityOptions,
    };
    use mtg_engine::mana::ManaPool;
    use rstest::rstest;

    const OTHER: PlayerId = PlayerId(1);

    fn priority(can_act: bool) -> DecisionKind {
        DecisionKind::Priority(PriorityOptions {
            playable_lands: Vec::new(),
            castable: Vec::new(),
            mana_abilities: Vec::new(),
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
            }),
            DecisionKind::DeclareAttackers(AttackOptions {
                attackers: Vec::new(),
            }),
            blockers(),
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

    fn decision(kind: DecisionKind) -> Decision {
        Decision {
            player: OTHER,
            kind,
        }
    }

    #[test]
    fn the_default_is_on_and_manual_is_off() {
        assert!(Pace::default().auto_pass);
        assert!(!std::hint::black_box(Pace::MANUAL).auto_pass);
    }

    #[rstest]
    #[case::idle_and_on(priority(false), &[true, true], true)]
    #[case::idle_but_off(priority(false), &[true, false], false)]
    #[case::can_act(priority(true), &[true, true], false)]
    #[case::unknown_seat_counts_as_off(priority(false), &[true], false)]
    fn skipping(#[case] kind: DecisionKind, #[case] flags: &[bool], #[case] skipped: bool) {
        assert_eq!(skippable(&decision(kind), flags), skipped);
    }

    #[test]
    fn no_other_kind_of_decision_is_ever_idle_or_skipped() {
        for kind in other_kinds() {
            let d = decision(kind);
            assert!(!idle(&d), "{d:?}");
            assert!(!skippable(&d, &[true, true]), "{d:?}");
        }
    }
}
