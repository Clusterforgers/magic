//! A skip of the next step or combat phase of a player's turn: "You skip your next untap step.", "Target player skips
//! their next draw step.", "That player skips their next untap step." and "Target player skips their next combat phase
//! this turn." (`Duration::UntilSkipped`).

use super::{Effect, Ref, Writer};
use crate::modify::{Duration, GameRule, Prohibited};
use crate::refs::Targetable;
use crate::trigger::Step;

impl Writer<'_> {
    /// The sentence of a rule that makes a player skip their next step or combat phase.
    pub(super) fn rmt3_skip(&mut self, effect: &Effect) -> Option<String> {
        let Effect::Rule {
            rule: GameRule::Cant { who, action },
            duration,
        } = effect
        else {
            return None;
        };
        let turn = match duration {
            Duration::UntilSkipped => "",
            Duration::UntilSkippedThisTurn => " this turn",
            _ => return None,
        };
        let what = match action {
            Prohibited::SkipCombat => "combat phase",
            Prohibited::SkipStep(Step::Untap) => "untap step",
            Prohibited::SkipStep(Step::Upkeep) => "upkeep",
            Prohibited::SkipStep(Step::Draw) => "draw step",
            Prohibited::SkipStep(Step::End) => "end step",
            _ => return None,
        };
        let player = |me: &Writer<'_>| {
            me.target_is(|what| matches!(what, Targetable::Player | Targetable::Opponent))
        };
        let subject = match who {
            Ref::You => return Some(format!("you skip your next {what}{turn}")),
            Ref::Target(0) if self.named && player(self) => "that player".to_owned(),
            other => self.player(other)?,
        };
        Some(format!("{subject} skips their next {what}{turn}"))
    }
}
