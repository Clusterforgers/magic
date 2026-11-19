//! The Satchel of a Delves seat: cards set up as objects in the owner's command zone. Its owner may cast a Satchel card
//! from there whenever they could cast it from their hand, for its mana cost (CR 601.2, 601.3). A once card follows the
//! normal rules after it is cast and never comes back. A reusable card (a boss's) returns to the Satchel instead of
//! going anywhere else as it leaves the stack or the battlefield, and costs {2} more for each earlier cast from the
//! Satchel this game, as the commander tax (CR 903.8). Every seat sees each Satchel.

use std::collections::{BTreeMap, BTreeSet};
use std::hash::{Hash, Hasher};

use mtg_core::ids::{ObjectId, PlayerId};

use crate::state::GameState;
use crate::zones::Zone;

/// The generic mana a reusable Satchel card costs more for each earlier cast from the Satchel.
pub const SATCHEL_TAX: u32 = 2;

/// The Satchel cards of a game, fixed when the game is set up, and the casts of the reusable ones.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Satchels {
    ids: BTreeSet<ObjectId>,
    /// The cards that return to the Satchel as they leave the stack or the battlefield.
    reusable: BTreeSet<ObjectId>,
    /// Times each reusable card was cast from the Satchel.
    casts: BTreeMap<ObjectId, u32>,
}

impl Hash for Satchels {
    fn hash<H: Hasher>(&self, state: &mut H) {
        if !self.ids.is_empty() {
            self.ids.hash(state);
        }
        if !self.reusable.is_empty() {
            1u8.hash(state);
            self.reusable.hash(state);
        }
        if !self.casts.is_empty() {
            2u8.hash(state);
            self.casts.hash(state);
        }
    }
}

impl Satchels {
    /// The Satchels of a game set up with these once and these reusable card objects.
    pub(crate) fn of(
        once: impl IntoIterator<Item = ObjectId>,
        reusable: impl IntoIterator<Item = ObjectId>,
    ) -> Satchels {
        let reusable: BTreeSet<ObjectId> = reusable.into_iter().collect();
        Satchels {
            ids: once.into_iter().chain(reusable.iter().copied()).collect(),
            reusable,
            casts: BTreeMap::new(),
        }
    }

    /// Whether `id` was set up as a Satchel card.
    pub(crate) fn contains(&self, id: ObjectId) -> bool {
        self.ids.contains(&id)
    }

    /// Whether any card was set up as a reusable Satchel card.
    pub(crate) fn any_reusable(&self) -> bool {
        !self.reusable.is_empty()
    }

    /// Whether `id` was set up as a reusable Satchel card.
    pub(crate) fn is_reusable(&self, id: ObjectId) -> bool {
        self.reusable.contains(&id)
    }

    /// A card was cast from `from`: a reusable card cast from the command zone counts for its tax.
    pub(crate) fn cast(&mut self, id: ObjectId, from: Zone) {
        if matches!(from, Zone::Command(_)) && self.reusable.contains(&id) {
            let count = self.casts.entry(id).or_insert(0);
            *count = count.saturating_add(1);
        }
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

    /// Whether `id` is a reusable Satchel card, one that returns to the Satchel after each cast.
    pub fn satchel_reusable(&self, id: ObjectId) -> bool {
        self.satchels.is_reusable(id)
    }

    /// How many times the reusable Satchel card `id` was cast from the Satchel this game.
    pub fn satchel_casts(&self, id: ObjectId) -> u32 {
        self.satchels.casts.get(&id).copied().unwrap_or(0)
    }

    /// The generic mana the reusable Satchel card `id` costs more to cast from the Satchel now: {2} for each earlier
    /// cast; 0 for any other card.
    pub fn satchel_tax(&self, id: ObjectId) -> u32 {
        if self.satchels.is_reusable(id) {
            self.satchel_casts(id).saturating_mul(SATCHEL_TAX)
        } else {
            0
        }
    }
}

/// The generic mana the spell `spell` costs more because it is a reusable card cast from the Satchel: {2} for each
/// earlier cast.
pub(crate) fn tax_for(state: &GameState, spell: ObjectId) -> u32 {
    if state.in_satchel(spell) {
        state.satchel_tax(spell)
    } else {
        0
    }
}

/// Where a reusable Satchel card that would move from `from` to `to` goes instead: back to its owner's Satchel as it
/// leaves the stack or the battlefield for any zone but the battlefield; `None` for every other move.
pub(crate) fn returns_to(
    state: &GameState,
    object: ObjectId,
    from: Zone,
    to: Zone,
) -> Option<Zone> {
    if !state.satchels.is_reusable(object)
        || !matches!(from, Zone::Stack | Zone::Battlefield)
        || matches!(to, Zone::Battlefield | Zone::Command(_) | Zone::Stack)
    {
        return None;
    }
    state.object(object).map(|card| Zone::Command(card.owner()))
}
