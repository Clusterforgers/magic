//! A tap and the rule that holds the same permanents down through their controller's next untap step, as one pair of
//! sentences that no joint splits.

use super::Writer;
use crate::compile::read::c7_untap::C7_UNTAP_TAILS;
use crate::effect::Effect;
use crate::modify::{Duration, Modification};
use crate::refs::Ref;

impl Writer<'_> {
    /// "tap target creature. It doesn't untap during its controller's next untap step" and the number of effects it
    /// covers.
    pub(super) fn c7_untap_held(&mut self, effects: &[Effect]) -> Option<(String, usize)> {
        let [
            tap @ Effect::Tap { what },
            Effect::Continuous {
                affected,
                mods,
                duration: Duration::NextUntapStep,
            },
            ..,
        ] = effects
        else {
            return None;
        };
        if affected != what || mods.as_slice() != [Modification::DoesntUntap] {
            return None;
        }
        if *what != Ref::Target(0) {
            return None;
        }
        let tail = C7_UNTAP_TAILS[usize::from(!self.at_most_one(what))];
        let tapped = self.effect(tap)?;
        Some((format!("{tapped}. {tail}"), 2))
    }
}
