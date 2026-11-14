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
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExtraTurn {
    /// Who takes it.
    pub player: PlayerId,
    /// Whether its untap step is skipped.
    pub skip_untap: bool,
    /// Whether power-up abilities can't be activated during it.
    pub no_power_up: bool,
}

// An extra turn that allows power-up abilities hashes as it did before the rule existed.
impl Hash for ExtraTurn {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.player.hash(state);
        self.skip_untap.hash(state);
        if self.no_power_up {
            self.no_power_up.hash(state);
        }
    }
}

/// The bit of a kind of step in the masks of the steps begun this turn.
const fn step_bit(step: Step) -> u16 {
    1 << (step as u16)
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
    /// Whether power-up abilities can't be activated this turn (an extra turn taken with that rule).
    pub(crate) no_power_up: bool,
    /// The combat phases begun so far this turn.
    pub(crate) combats: u32,
    /// The phases still to begin when the added ones come after the current phase (CR 500.8), each as the step it
    /// begins with, the added ones first and the phase of the regular sequence they hold up last; empty while the
    /// turn runs its regular sequence.
    pub(crate) ahead: Vec<Step>,
    /// For each entry of `ahead`, whether it is a step on its own rather than the first step of a phase; empty while
    /// none is.
    pub(crate) r5500_lone: Vec<bool>,
    /// Whether the current step began on its own from `ahead`, so the phase ends with it.
    pub(crate) r5500_in_lone: bool,
    /// The steps added right after the current step (an additional upkeep step), the next first; empty while none is.
    pub(crate) r306_steps: Vec<Step>,
    /// The kinds of step begun this turn, one bit each in turn order.
    pub(crate) r306_seen: u16,
    /// The kinds of step begun this turn more than once (an added upkeep step), one bit each.
    pub(crate) r306_again: u16,
    /// Whether the turn was ended by an effect (CR 723): the cleanup step began while a spell or ability resolved, and no
    /// player gets priority when it finishes.
    pub(crate) ending: bool,
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
        if !self.r306_steps.is_empty() {
            self.r306_steps.hash(state);
        }
        if !self.r5500_lone.is_empty() || self.r5500_in_lone {
            self.r5500_lone.hash(state);
            self.r5500_in_lone.hash(state);
        }
        if self.r306_again != 0 {
            self.r306_again.hash(state);
        }
        if self.no_power_up {
            self.no_power_up.hash(state);
        }
        if self.ending {
            self.ending.hash(state);
        }
    }
}

impl TurnState {
    /// The turn state before the first turn, with `first` as the active player.
    pub(crate) const fn pregame(first: PlayerId) -> TurnState {
        TurnState {
            number: 0,
            active: first,
            step: Step::Untap,
            lands_played: 0,
            round: 0,
            regular: first,
            extra: false,
            no_power_up: false,
            combats: 0,
            ahead: Vec::new(),
            r306_steps: Vec::new(),
            r5500_lone: Vec::new(),
            r5500_in_lone: false,
            r306_seen: 0,
            r306_again: 0,
            ending: false,
        }
    }

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

    /// Whether a step of the current step's kind began earlier this turn (`Condition::FirstStep` is its negation).
    pub fn step_began_again(&self) -> bool {
        self.r306_again & step_bit(self.step) != 0
    }

    /// Notes that `step` begins: a kind begun before this turn is begun again.
    pub(crate) fn r306_note_step(&mut self, step: Step) {
        let bit = step_bit(step);
        if self.r306_seen & bit != 0 {
            self.r306_again |= bit;
        }
        self.r306_seen |= bit;
    }

    /// The steps added right after the current one, the next first (`Effect::AddStep`).
    pub fn steps_ahead(&self) -> &[Step] {
        &self.r306_steps
    }

    /// Whether the current step began on its own after a phase (an additional upkeep step after this phase): the
    /// steps of its phase do not follow it.
    pub const fn step_is_lone(&self) -> bool {
        self.r5500_in_lone
    }

    /// Adds `steps` in front of the phases ahead, each a step on its own when `lone`.
    pub(crate) fn r5500_add_ahead(&mut self, steps: &[Step], lone: bool) {
        if lone && self.r5500_lone.is_empty() {
            self.r5500_lone = vec![false; self.ahead.len()];
        }
        self.ahead.splice(0..0, steps.iter().copied());
        if !self.r5500_lone.is_empty() {
            self.r5500_lone.splice(0..0, steps.iter().map(|_| lone));
        }
    }

    /// Spends the first entry of the phases ahead as `step` begins from it, noting whether the step is on its own.
    pub(crate) fn r5500_begin_ahead(&mut self) {
        self.ahead.remove(0);
        self.r5500_in_lone = !self.r5500_lone.is_empty() && self.r5500_lone.remove(0);
        if !self.r5500_lone.contains(&true) {
            self.r5500_lone.clear();
        }
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
