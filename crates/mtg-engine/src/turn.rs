//! Turn structure: phases, steps and the per-turn counters.

use std::hash::{Hash, Hasher};

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

impl Phase {
    /// The step the next phase of the regular sequence begins with; `None` after the ending phase.
    pub const fn next_start(self) -> Option<Step> {
        match self {
            Phase::Beginning => Some(Step::Main1),
            Phase::PrecombatMain => Some(Step::BeginCombat),
            Phase::Combat => Some(Step::Main2),
            Phase::PostcombatMain => Some(Step::End),
            Phase::Ending => None,
        }
    }
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

/// An extra turn waiting to be taken (CR 500.7).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ExtraTurn {
    /// Who takes it.
    pub player: PlayerId,
    /// Whether its untap step is skipped.
    pub skip_untap: bool,
}

/// Where the game is within the turn sequence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TurnState {
    pub(crate) number: u32,
    pub(crate) active: PlayerId,
    pub(crate) step: Step,
    pub(crate) lands_played: u32,
    pub(crate) round: u32,
    /// The seat whose regular turn this is: an extra turn keeps the seat it was added after, so the turn order goes on
    /// from there (CR 500.7).
    pub(crate) regular: PlayerId,
    /// Whether this turn is an extra turn.
    pub(crate) extra: bool,
    /// The combat phases begun so far this turn.
    pub(crate) combats: u32,
    /// The phases still to begin when the added ones come after the current phase (CR 500.8), each as the step it
    /// begins with, the added ones first and the phase of the regular sequence they hold up last; empty while the
    /// turn runs its regular sequence.
    pub(crate) ahead: Vec<Step>,
}

// The fields a game without extra turns or phases never sets add nothing, so such a game hashes as it did before.
impl Hash for TurnState {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.number.hash(state);
        self.active.hash(state);
        self.step.hash(state);
        self.lands_played.hash(state);
        self.round.hash(state);
        if self.regular != self.active || self.extra {
            self.regular.hash(state);
            self.extra.hash(state);
        }
        if self.combats > 1 {
            self.combats.hash(state);
        }
        if !self.ahead.is_empty() {
            self.ahead.hash(state);
        }
    }
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

    /// Whether this turn is an extra turn (CR 500.7).
    pub const fn is_extra(&self) -> bool {
        self.extra
    }

    /// The seat whose regular turn this is; for an extra turn, the seat it was added after.
    pub const fn regular(&self) -> PlayerId {
        self.regular
    }

    /// The combat phases begun so far this turn, the current one included.
    pub const fn combats(&self) -> u32 {
        self.combats
    }

    /// The phases that begin after the current one before the regular sequence goes on, each as its first step (CR 500.8).
    pub fn phases_ahead(&self) -> &[Step] {
        &self.ahead
    }

    /// The round of the turn, from 1: a round is one turn of each seat in play, from the first player's seat round to
    /// the seat before it (0 before the first turn).
    pub const fn round(&self) -> u32 {
        self.round
    }
}

/// Lands a player may play per turn unless the house says otherwise.
pub const LAND_DROPS_PER_TURN: u32 = 1;

/// Cards a player draws in the draw step unless the house says otherwise.
pub const DRAW_STEP_CARDS: usize = 1;

/// The poison counters that make a player lose unless the format says otherwise (CR 704.5c).
pub const POISON_LOSS: u32 = 10;

/// The maximum hand size a format enforces at cleanup unless it says otherwise.
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
