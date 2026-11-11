//! rmt2: a prevention shield divided among targets ("prevent the next 5 damage that would be dealt this turn to any
//! number of targets, divided as you choose").

use super::divided::among_text;
use super::{Writer, points_text};
use crate::effect::Effect;
use crate::modify::Duration;
use crate::refs::Ref;
use crate::trigger::{ReplaceBy, ReplaceWhen};
use crate::value::Amount;

impl Writer<'_> {
    /// The shield of each target's share of the divided amount for the turn.
    pub(super) fn rmt2_divided_prevention(&mut self, effect: &Effect) -> Option<String> {
        let Effect::Shield {
            when:
                ReplaceWhen::Damage {
                    to: Some(Ref::Target(0)),
                    source,
                    combat: None,
                },
            by: ReplaceBy::PreventCounted(Amount::Divided),
            duration: Duration::UntilEndOfTurn,
        } = effect
        else {
            return None;
        };
        if !source.0.is_empty() || self.named {
            return None;
        }
        let spec = self.targets.first()?;
        let total = points_text(spec.divide.as_ref()?)?;
        let among = among_text(spec)?;
        self.named = true;
        Some(format!(
            "prevent the next {total} damage that would be dealt this turn to {among}, divided as you choose"
        ))
    }
}
