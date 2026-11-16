//! The Satchel of a Delves hero: cards the hero earned in the run, set up as objects in the owner's command zone. Its
//! owner may cast a Satchel card from there whenever they could cast it from their hand, for its mana cost (CR 601.2,
//! 601.3); once cast it follows the normal rules and never comes back to the Satchel. Every seat sees it.

use std::collections::BTreeSet;
use std::hash::{Hash, Hasher};

use mtg_core::ids::{ObjectId, PlayerId};

use crate::state::GameState;
use crate::zones::Zone;

/// The Satchel cards of a game, fixed when the game is set up.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Satchels {
    ids: BTreeSet<ObjectId>,
}

impl Hash for Satchels {
    fn hash<H: Hasher>(&self, state: &mut H) {
        if !self.ids.is_empty() {
            self.ids.hash(state);
        }
    }
}

impl Satchels {
    /// The Satchels of a game set up with these card objects.
    pub(crate) fn of(ids: impl IntoIterator<Item = ObjectId>) -> Satchels {
        Satchels {
            ids: ids.into_iter().collect(),
        }
    }

    /// Whether `id` was set up as a Satchel card.
    pub(crate) fn contains(&self, id: ObjectId) -> bool {
        self.ids.contains(&id)
    }
}

impl GameState {
    /// Whether `id` is a card in its owner's Satchel now: set up there and still in the command zone.
    pub fn in_satchel(&self, id: ObjectId) -> bool {
        self.satchels.contains(id)
            && self
                .object(id)
                .is_some_and(|object| matches!(object.zone(), Zone::Command(_)))
    }

    /// The cards in `seat`'s Satchel, in the order they were set up.
    pub fn satchel(&self, seat: PlayerId) -> Vec<ObjectId> {
        self.zone(Zone::Command(seat))
            .iter()
            .copied()
            .filter(|id| self.satchels.contains(*id))
            .collect()
    }
}
