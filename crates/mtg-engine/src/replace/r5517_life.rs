//! "If a spell or ability would cause its controller to gain life" (`ReplaceWhen::LifeGain` with `caused_own`): only
//! life a resolving spell or ability gives its own controller; the life of lifelink is caused by the damage, not by a
//! spell or ability (Rain of Gore ruling: it does not apply to combat damage from a creature with lifelink).

use mtg_cards::trigger::ReplaceWhen;

use super::{Item, Pass, Unit};
use crate::events::Event;

impl Pass<'_> {
    /// Whether a replacement of life gained with `caused_own` covers the gain at slot `at`; any other replacement passes.
    pub(super) fn r5517_caused_own(&self, at: usize, when: &ReplaceWhen, unit: &Unit) -> bool {
        let ReplaceWhen::LifeGain {
            caused_own: true, ..
        } = when
        else {
            return true;
        };
        let Unit::Life { player } = unit else {
            return true;
        };
        let caused = matches!(self.putter, Some((by, true)) if by == *player);
        caused && !self.r5517_lifelink_gain(at, *player)
    }

    /// Whether the life gained at slot `at` is the lifelink of the damage dealt just before it.
    fn r5517_lifelink_gain(&self, at: usize, player: mtg_core::ids::PlayerId) -> bool {
        let Some(before) = at.checked_sub(1).and_then(|before| self.slots.get(before)) else {
            return false;
        };
        let Item::One(Event::DamageDealt { source, .. }) = &before.item else {
            return false;
        };
        crate::damage::Dealer::live(self.state, *source).lifelink_for(player)
    }
}
