//! A shield over the next draw or loss of the game ("the next time you would draw a card this turn, ... instead", "the
//! next time you would lose the game this turn, instead ...", CR 614.1a): it replaces one such event of the players it
//! names and then ends.

use mtg_cards::trigger::{ReplaceBy, ReplaceWhen};

use super::Pass;
use crate::rules::{Rule, RuleEffect};

/// Whether the rule is a shield over the next draw or loss of the game.
pub(super) fn is_next(rule: &Rule) -> bool {
    matches!(
        rule,
        Rule::Replaces {
            when: ReplaceWhen::Draw { .. } | ReplaceWhen::LoseGame { .. },
            by: ReplaceBy::InsteadNext(_),
            ..
        }
    )
}

/// The replacement a shield applies: the steps of a shield over the next event run as an `Instead`.
pub(super) fn rmt5_next_as_instead(by: &ReplaceBy) -> ReplaceBy {
    match by {
        ReplaceBy::InsteadNext(effects) => ReplaceBy::Instead(effects.clone()),
        other => other.clone(),
    }
}

impl Pass<'_> {
    /// Whether the shield over the next event has already replaced one in this batch.
    pub(super) fn rmt5_next_spent(&self, effect: &RuleEffect) -> bool {
        is_next(&effect.rule) && self.spent.contains(&effect.id)
    }
}
