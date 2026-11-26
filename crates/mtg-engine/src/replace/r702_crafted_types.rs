//! `EntersChoice::R702CraftedSharedType` (CR 702.167c): "As it enters, choose a card type shared among two exiled cards
//! used to craft it." The choice is among the card types every material in exile has.

use mtg_core::card::CardTypes;
use mtg_core::ids::ObjectId;

use super::Pass;
use crate::choose::values::CARD_TYPES;

impl Pass<'_> {
    /// The card types all the cards used to craft `entering` share, as they are in exile; none when the craft ability is
    /// not what returns it, or when no material is still in exile.
    pub(super) fn r702_crafted_shared_types(&self, entering: ObjectId) -> CardTypes {
        let materials = crate::fold::r6702_craft_materials(self.state, entering);
        if materials.is_empty() {
            return CardTypes::default();
        }
        CARD_TYPES
            .into_iter()
            .filter(|kind| {
                materials.iter().all(|card| {
                    crate::characteristics::characteristics(self.state, card.id)
                        .is_some_and(|chars| chars.types.contains(*kind))
                })
            })
            .fold(CardTypes::default(), CardTypes::with)
    }
}
