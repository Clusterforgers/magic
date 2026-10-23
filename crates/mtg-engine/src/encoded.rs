//! Cipher links (CR 702.99): the cards in exile that are encoded on creatures.

use mtg_core::ids::ObjectId;

use crate::object::GameObject;
use crate::state::GameState;
use crate::zones::Zone;

impl GameState {
    /// The creature `card` is encoded on, while the link holds: the card lies in exile and the creature is still the
    /// permanent it was when the card was encoded (a creature that left the battlefield is a new object, CR 400.7).
    pub fn encoded_creature(&self, card: ObjectId) -> Option<ObjectId> {
        let card = self
            .object(card)
            .filter(|card| card.zone() == Zone::Exile)?;
        let carrier = card.encoded_on()?;
        self.object(carrier.id)
            .filter(|creature| {
                creature.zone() == Zone::Battlefield && creature.counter() == carrier.counter
            })
            .map(GameObject::id)
    }
}
