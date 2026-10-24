//! A prepare card that enters prepared: "This creature enters prepared."

use mtg_cards::definition::Layout;
use mtg_core::ids::{ObjectId, PlayerId};

use super::{Item, Pass, Slot};
use crate::events::Event;

impl Pass<'_> {
    /// The permanent `object` becomes prepared right after it enters: the designation is logged after its `Entered`, so
    /// nothing sees it without. A card that is no prepare card, or that is already prepared in the batch, enters as it was.
    pub(super) fn enters_prepared(&mut self, at: usize, object: ObjectId, controller: PlayerId) {
        let prepares = self
            .state
            .object(object)
            .is_some_and(|card| card.whole_card().layout == Layout::Prepare);
        let done = self.slots.iter().any(
            |slot| matches!(&slot.item, Item::One(Event::Prepared { object: o }) if *o == object),
        );
        if !prepares || done {
            return;
        }
        let entered = self.entered_at(at, object, controller);
        self.slots.insert(
            entered + 1,
            Slot {
                item: Item::One(Event::Prepared { object }),
                applied: Vec::new(),
                frozen: true,
            },
        );
    }
}
