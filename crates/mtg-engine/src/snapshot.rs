//! Last known information: an object as it was, kept after it has changed or gone (CR 113.7a, 608.2h).

use mtg_cards::value::CounterKind;
use mtg_core::ids::{ObjectId, PlayerId};
use serde::{Deserialize, Serialize};

use crate::characteristics::{Characteristics, characteristics};
use crate::object::ObjectRef;
use crate::state::GameState;
use crate::zones::Zone;

/// An object as it was at one moment; the engine keeps one in the places listed in `docs/cards-dsl.md` only.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct Snapshot {
    /// The object.
    pub id: ObjectId,
    /// Its zone-change counter then: the snapshot describes this object and no later one.
    pub counter: u32,
    /// Who controlled it.
    pub controller: PlayerId,
    /// Who owned it.
    pub owner: PlayerId,
    /// Where it was.
    pub zone: Zone,
    /// What it was, with every continuous effect applied.
    pub characteristics: Characteristics,
    /// The counters on it, by kind (none until counters exist).
    pub counters: Vec<(CounterKind, u32)>,
    /// The objects attached to it (none until attachments exist).
    pub attachments: Vec<ObjectId>,
}

impl Snapshot {
    /// The reference that names the object this snapshot describes.
    pub const fn reference(&self) -> ObjectRef {
        ObjectRef {
            id: self.id,
            counter: self.counter,
        }
    }
}

/// The object `id` as it is now, or `None` when there is no such object.
pub fn snapshot(state: &GameState, id: ObjectId) -> Option<Snapshot> {
    let object = state.object(id)?;
    Some(Snapshot {
        id,
        counter: object.counter(),
        controller: object.controller(),
        owner: object.owner(),
        zone: object.zone(),
        characteristics: characteristics(state, id)?,
        counters: Vec::new(),
        attachments: Vec::new(),
    })
}
