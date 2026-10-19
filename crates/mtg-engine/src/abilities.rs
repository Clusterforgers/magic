//! Abilities as the engine names and stores them: a reference to one, and what one becomes on the stack.

use std::hash::{Hash, Hasher};
use std::sync::Arc;

use mtg_cards::ability::Ability;
use mtg_core::ids::{ObjectId, PlayerId};
use serde::{Deserialize, Serialize};

use crate::choose::{Choices, Target};
use crate::object::ObjectRef;
use crate::snapshot::Snapshot;

/// An ability of a permanent, named by the permanent and the ability's index among its card's rules.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct AbilityRef {
    /// The permanent with the ability.
    pub object: ObjectId,
    /// Index into the rules of its card (`CardDefinition::rules`).
    pub ability: usize,
}

/// What made a triggered ability trigger: the objects and players the event was about.
///
/// Objects are kept as they were when the event happened (CR 603.10a), so the ability can still read them.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct Cause {
    /// The things the event was about (`Ref::Triggering`).
    pub subjects: Vec<Target>,
    /// The objects among them, as they were.
    pub snapshots: Vec<Snapshot>,
}

/// An ability put on the stack, activated by its controller or triggered: what the `AbilityPutOnStack` event says.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct PutOnStack {
    /// The ability's new object.
    pub object: ObjectId,
    /// The object it comes from, as it was.
    pub source: ObjectRef,
    /// The source as it was when the ability was put on the stack (a cost may have moved it since).
    pub snapshot: Snapshot,
    /// Its index among the rules of the source's card.
    pub ability: usize,
    /// Who controls it.
    pub controller: PlayerId,
    /// What the controller announced: targets, modes and the like.
    #[serde(default, skip_serializing_if = "Choices::is_empty")]
    #[cfg_attr(feature = "ts", ts(as = "Option<Choices>", optional))]
    pub choices: Choices,
    /// What triggered it, for a triggered ability.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub cause: Option<Cause>,
}

/// An activated or triggered ability on the stack: it exists on its own once put there (CR 113.7a).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AbilityObject {
    pub(crate) source: ObjectRef,
    pub(crate) snapshot: Snapshot,
    pub(crate) index: usize,
    pub(crate) rules: Arc<Ability>,
    pub(crate) cause: Option<Cause>,
}

impl AbilityObject {
    /// The object the ability came from, as it was when the ability was put on the stack.
    pub const fn source(&self) -> ObjectRef {
        self.source
    }

    /// The source as it was then (last known information).
    pub const fn snapshot(&self) -> &Snapshot {
        &self.snapshot
    }

    /// Its index among the rules of the source's card.
    pub const fn index(&self) -> usize {
        self.index
    }

    /// The rules it runs; the ability needs no source to resolve.
    pub fn rules(&self) -> &Ability {
        &self.rules
    }

    /// What triggered it, for a triggered ability.
    pub const fn cause(&self) -> Option<&Cause> {
        self.cause.as_ref()
    }
}

// The rules follow from the source's card and the index, so hashing them would only cost time.
impl Hash for AbilityObject {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.source.hash(state);
        self.snapshot.hash(state);
        self.index.hash(state);
        self.cause.hash(state);
    }
}
