//! Game objects: cards (and later tokens and abilities) with their zone and permanent state.

use std::hash::{Hash, Hasher};
use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_core::card::CardTypes;
use mtg_core::ids::{ObjectId, PlayerId};

use crate::hash::stable_hash;
use crate::zones::Zone;

/// State that only exists while an object is on the battlefield.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PermanentState {
    pub(crate) tapped: bool,
    pub(crate) damage: u32,
    pub(crate) summoning_sick: bool,
}

impl PermanentState {
    /// Whether the permanent is tapped.
    pub const fn tapped(&self) -> bool {
        self.tapped
    }

    /// Damage marked on the permanent this turn (tracked apart from toughness).
    pub const fn damage(&self) -> u32 {
        self.damage
    }

    /// Whether the controller has not yet controlled it since their turn began.
    pub const fn summoning_sick(&self) -> bool {
        self.summoning_sick
    }
}

/// A card in the game with its current zone, controller and permanent state.
#[derive(Debug, Clone)]
pub struct GameObject {
    pub(crate) id: ObjectId,
    pub(crate) owner: PlayerId,
    pub(crate) controller: PlayerId,
    pub(crate) def: Arc<CardDefinition>,
    pub(crate) def_fingerprint: u64,
    pub(crate) base_types: CardTypes,
    pub(crate) zone: Zone,
    pub(crate) permanent: Option<PermanentState>,
}

impl GameObject {
    pub(crate) fn new(id: ObjectId, owner: PlayerId, def: Arc<CardDefinition>, zone: Zone) -> Self {
        GameObject {
            id,
            owner,
            controller: owner,
            def_fingerprint: stable_hash(&*def),
            base_types: def.type_set(),
            def,
            zone,
            permanent: None,
        }
    }

    /// The object's id.
    pub const fn id(&self) -> ObjectId {
        self.id
    }

    /// The player who owns the card.
    pub const fn owner(&self) -> PlayerId {
        self.owner
    }

    /// The player who controls it (the owner unless on the battlefield or stack).
    pub const fn controller(&self) -> PlayerId {
        self.controller
    }

    /// The printed card definition.
    pub fn card(&self) -> &CardDefinition {
        &self.def
    }

    /// The printed card types (no effects applied).
    pub const fn base_types(&self) -> CardTypes {
        self.base_types
    }

    /// The zone the object is in.
    pub const fn zone(&self) -> Zone {
        self.zone
    }

    /// Permanent state when on the battlefield.
    pub const fn permanent(&self) -> Option<&PermanentState> {
        self.permanent.as_ref()
    }
}

impl PartialEq for GameObject {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
            && self.owner == other.owner
            && self.controller == other.controller
            && self.def_fingerprint == other.def_fingerprint
            && self.zone == other.zone
            && self.permanent == other.permanent
    }
}

impl Eq for GameObject {}

// The definition is hashed once at creation; hashing it per state hash would dominate runtime.
impl Hash for GameObject {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.id.hash(state);
        self.owner.hash(state);
        self.controller.hash(state);
        self.def_fingerprint.hash(state);
        self.zone.hash(state);
        self.permanent.hash(state);
    }
}
