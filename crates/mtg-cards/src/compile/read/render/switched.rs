//! A switch of power and toughness: "Switch target creature's power and toughness until end of turn.", "Switch the power
//! and toughness of each of any number of target creatures until end of turn."

use super::Writer;
use crate::modify::{Duration, Modification};
use crate::refs::{Ref, Targetable};
use crate::value::Amount;

impl Writer<'_> {
    /// The sentence for a switch of the card, of the one target or of any number of targets, for the turn.
    pub(super) fn switched(
        &mut self,
        affected: &Ref,
        mods: &[Modification],
        duration: Duration,
    ) -> Option<String> {
        let [Modification::SwitchPT] = mods else {
            return None;
        };
        (duration == Duration::UntilEndOfTurn).then_some(())?;
        if *affected == Ref::This {
            return Some("switch ~'s power and toughness until end of turn".to_owned());
        }
        (*affected == Ref::Target(0)
            && self.target_is(|what| matches!(what, Targetable::Object(_))))
        .then_some(())?;
        if self.single(affected) {
            return Some(format!(
                "switch {}'s power and toughness until end of turn",
                self.name_target()?
            ));
        }
        // "each of any number of target creatures": none to every number of them.
        let from_none = self
            .targets
            .first()
            .is_some_and(|spec| spec.bounds.min == Amount::Fixed(0));
        (from_none && !self.at_most_one(affected)).then_some(())?;
        Some(format!(
            "switch the power and toughness of each of {} until end of turn",
            self.name_target()?
        ))
    }
}
