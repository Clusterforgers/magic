//! The day a permanent starts as it enters (CR 726.2): "If it's neither day nor night, it becomes day as ~ enters."

use mtg_cards::value::DayNight;

use super::{Item, Origin, Pass, Slot};
use crate::events::Event;

impl Pass<'_> {
    /// The enter event at `at` brings the day with it when it is neither day nor night: the change is logged just
    /// before the permanent enters, so nothing sees the permanent without it. Several permanents entering together
    /// start the day once.
    pub(super) fn day_if_neither(&mut self, at: usize, applied: Vec<Origin>) {
        let started = self.state.day_night().is_some()
            || self
                .slots
                .iter()
                .any(|slot| matches!(slot.item, Item::One(Event::DayNightChanged { .. })));
        if started {
            self.slots[at].applied = applied;
            return;
        }
        self.slots.insert(
            at,
            Slot {
                item: Item::One(Event::DayNightChanged { to: DayNight::Day }),
                applied: Vec::new(),
                frozen: true,
            },
        );
        // The enter event moved one place on.
        self.slots[at + 1].applied = applied;
    }
}
