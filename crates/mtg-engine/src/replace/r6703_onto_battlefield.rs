//! "If an artifact or creature spell is countered this way, put that card onto the battlefield under your control
//! instead of into its owner's graveyard": a countered spell's card is moved to the battlefield and enters under the
//! shield's controller (`ReplaceBy::BattlefieldUnderYours`, Desertion).

use mtg_core::ids::{ObjectId, PlayerId};

use super::{Item, Origin, Pass, Slot};
use crate::events::Event;
use crate::zones::Zone;

impl Pass<'_> {
    /// Sends the countered card of the slot `at` from `from` to the battlefield, followed by its entering under
    /// `controller`; the move is then judged as an entering, so the replacements of entering apply to it.
    pub(super) fn r6703_onto_battlefield(
        &mut self,
        at: usize,
        (object, from): (ObjectId, Zone),
        controller: PlayerId,
        applied: &[Origin],
    ) {
        self.redirect(at, object, from, Zone::Battlefield, applied);
        let moved = self.slots.iter().position(|slot| {
            matches!(
                &slot.item,
                Item::One(Event::ZoneChanged { object: o, to: Zone::Battlefield, .. }) if *o == object
            )
        });
        let Some(moved) = moved else {
            return;
        };
        self.slots.insert(
            moved + 1,
            Slot {
                frozen: false,
                item: Item::One(Event::Entered {
                    object,
                    tapped: false,
                    controller,
                    chosen: None,
                }),
                applied: applied.to_vec(),
            },
        );
    }
}
