//! Turn structure: phases, steps and the per-turn counters.

use mtg_core::ids::PlayerId;
use serde::{Deserialize, Serialize};

/// A phase of the turn (CR 500.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Phase {
    /// Untap, upkeep and draw.
    Beginning,
    /// First main phase.
    PrecombatMain,
    /// Combat.
    Combat,
    /// Second main phase.
    PostcombatMain,
    /// End step and cleanup.
    Ending,
}

/// A step or main phase of a turn, in turn order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub enum Step {
    /// Untap step.
    Untap,
    /// Upkeep step.
    Upkeep,
    /// Draw step.
    Draw,
    /// Precombat main phase.
    Main1,
    /// Beginning of combat step.
    BeginCombat,
    /// Declare attackers step.
    DeclareAttackers,
    /// Declare blockers step.
    DeclareBlockers,
    /// First-strike combat damage step; only occurs when a first or double striker is in combat.
    FirstStrikeDamage,
    /// Regular combat damage step.
    CombatDamage,
    /// End of combat step.
    EndCombat,
    /// Postcombat main phase.
    Main2,
    /// End step.
    End,
    /// Cleanup step.
    Cleanup,
}

impl Step {
    /// All steps in order.
    pub const ALL: [Step; 13] = [
        Step::Untap,
        Step::Upkeep,
        Step::Draw,
        Step::Main1,
        Step::BeginCombat,
        Step::DeclareAttackers,
        Step::DeclareBlockers,
        Step::FirstStrikeDamage,
        Step::CombatDamage,
        Step::EndCombat,
        Step::Main2,
        Step::End,
        Step::Cleanup,
    ];

    /// The step after this one in the full sequence (before skip rules); `None` after cleanup.
    pub fn next(self) -> Option<Step> {
        Step::ALL.get(self as usize + 1).copied()
    }

    /// The phase this step belongs to.
    pub const fn phase(self) -> Phase {
        match self {
            Step::Untap | Step::Upkeep | Step::Draw => Phase::Beginning,
            Step::Main1 => Phase::PrecombatMain,
            Step::BeginCombat
            | Step::DeclareAttackers
            | Step::DeclareBlockers
            | Step::FirstStrikeDamage
            | Step::CombatDamage
            | Step::EndCombat => Phase::Combat,
            Step::Main2 => Phase::PostcombatMain,
            Step::End | Step::Cleanup => Phase::Ending,
        }
    }

    /// Whether players normally receive priority in this step (CR 502.4, 514.3).
    pub const fn grants_priority(self) -> bool {
        !matches!(self, Step::Untap | Step::Cleanup)
    }

    /// Whether this is a main phase (sorcery-speed timing).
    pub const fn is_main(self) -> bool {
        matches!(self, Step::Main1 | Step::Main2)
    }
}

/// Where the game is within the turn sequence.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TurnState {
    pub(crate) number: u32,
    pub(crate) active: PlayerId,
    pub(crate) step: Step,
    pub(crate) lands_played: u32,
}

impl TurnState {
    /// Turn number, starting at 1 (0 before the first turn).
    pub const fn number(&self) -> u32 {
        self.number
    }

    /// The active player.
    pub const fn active(&self) -> PlayerId {
        self.active
    }

    /// The current step.
    pub const fn step(&self) -> Step {
        self.step
    }

    /// Lands the active player has played this turn.
    pub const fn lands_played(&self) -> u32 {
        self.lands_played
    }
}

/// Lands a player may play per turn (no effects change this yet).
pub const LAND_DROPS_PER_TURN: u32 = 1;

/// Maximum hand size enforced at cleanup (no effects change this yet).
pub const MAX_HAND_SIZE: usize = 7;

#[cfg(test)]
mod tests {
    //! Goal: the step table is a total, ordered sequence with the right phases and priority rules.
    use super::*;
    use rstest::rstest;

    #[test]
    fn steps_chain_from_untap_to_cleanup() {
        let mut seen = vec![Step::Untap];
        while let Some(next) = seen.last().and_then(|s| s.next()) {
            seen.push(next);
        }
        assert_eq!(seen, Step::ALL);
        assert!(Step::ALL.windows(2).all(|w| w[0] < w[1]));
    }

    #[rstest]
    #[case(Step::Untap, Phase::Beginning, false)]
    #[case(Step::Upkeep, Phase::Beginning, true)]
    #[case(Step::Draw, Phase::Beginning, true)]
    #[case(Step::Main1, Phase::PrecombatMain, true)]
    #[case(Step::BeginCombat, Phase::Combat, true)]
    #[case(Step::DeclareAttackers, Phase::Combat, true)]
    #[case(Step::DeclareBlockers, Phase::Combat, true)]
    #[case(Step::FirstStrikeDamage, Phase::Combat, true)]
    #[case(Step::CombatDamage, Phase::Combat, true)]
    #[case(Step::EndCombat, Phase::Combat, true)]
    #[case(Step::Main2, Phase::PostcombatMain, true)]
    #[case(Step::End, Phase::Ending, true)]
    #[case(Step::Cleanup, Phase::Ending, false)]
    fn phase_and_priority_table(#[case] step: Step, #[case] phase: Phase, #[case] priority: bool) {
        assert_eq!(step.phase(), phase);
        assert_eq!(step.grants_priority(), priority);
    }

    #[test]
    fn only_the_two_main_phases_are_main() {
        let mains: Vec<_> = Step::ALL.into_iter().filter(|s| s.is_main()).collect();
        assert_eq!(mains, vec![Step::Main1, Step::Main2]);
    }
}
