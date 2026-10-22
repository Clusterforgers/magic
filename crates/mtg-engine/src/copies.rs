//! Copies (CR 707): the copiable values of an object, the copy effect that gives them to a permanent, and the event
//! that begins it.
//!
//! A copy effect is the first layer (CR 613.1a): the permanent's card is replaced by the copiable values the effect
//! names, and every other effect (counters, auras, `becomes` effects, type changes) applies on top through the layers
//! as usual. Later copy effects replace earlier ones (CR 613.7). The values are a [`CardDefinition`], so the name,
//! mana cost, types, abilities, power, toughness and loyalty of the copy are read from it wherever the card is.

use std::hash::{Hash, Hasher};
use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::definition::Layout;
use mtg_core::ids::ObjectId;
use serde::{Deserialize, Serialize};

use crate::error::EngineError;
use crate::fold::invalid;
use crate::hash::stable_hash;
use crate::object::{GameObject, ObjectKind, ObjectRef};
use crate::snapshot::Snapshot;
use crate::state::GameState;

/// A copy effect on a permanent: the object it copies and the copiable values it gives (CR 707.2).
#[derive(Debug, Clone)]
pub struct CopyEffect {
    of: ObjectRef,
    definition: Arc<CardDefinition>,
    fingerprint: u64,
    timestamp: u64,
}

impl CopyEffect {
    /// A copy of `of` with these copiable values, made at `timestamp`.
    pub(crate) fn new(of: ObjectRef, definition: Arc<CardDefinition>, timestamp: u64) -> Self {
        CopyEffect {
            of,
            fingerprint: stable_hash(&*definition),
            definition,
            timestamp,
        }
    }

    /// The object copied, as it was when the copy effect began.
    pub const fn of(&self) -> ObjectRef {
        self.of
    }

    /// The copiable values the effect gives, exceptions included (CR 707.9b).
    pub fn definition(&self) -> &CardDefinition {
        &self.definition
    }

    /// The copiable values, shared with the copy effect.
    pub(crate) fn shared(&self) -> Arc<CardDefinition> {
        Arc::clone(&self.definition)
    }

    /// The timestamp of the copy effect: a later one replaces an earlier one (CR 613.7).
    pub const fn timestamp(&self) -> u64 {
        self.timestamp
    }
}

impl PartialEq for CopyEffect {
    fn eq(&self, other: &Self) -> bool {
        self.of == other.of
            && self.fingerprint == other.fingerprint
            && self.timestamp == other.timestamp
    }
}

impl Eq for CopyEffect {}

// The values hash once, when the effect is made.
impl Hash for CopyEffect {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.of.hash(state);
        self.fingerprint.hash(state);
        self.timestamp.hash(state);
    }
}

/// A permanent became a copy of another object (CR 707.2): its copiable values are now those of the copy effect, with a
/// timestamp of its own. It follows the move to the battlefield in the batch of a permanent that enters as a copy
/// (CR 614.1c), so the copy is what enters and what triggers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct BecameCopy {
    /// The permanent that is the copy.
    pub object: ObjectId,
    /// The object it copies, as it was.
    pub of: ObjectRef,
    /// The copiable values, so a log folds to the same game with no card data at hand.
    #[cfg_attr(feature = "ts", ts(skip))]
    pub definition: Arc<CardDefinition>,
}

/// Folds the event: the permanent takes the copy effect, which replaces any earlier one.
pub(crate) fn fold(state: &mut GameState, event: &BecameCopy) -> Result<(), EngineError> {
    let timestamp = state.next_timestamp;
    let object = state
        .objects
        .get_mut(&event.object)
        .ok_or_else(|| invalid(format!("{:?} does not exist", event.object)))?;
    if object.kind() == ObjectKind::Ability {
        return Err(invalid("an ability on the stack is not copied into"));
    }
    let permanent = object
        .permanent
        .as_mut()
        .ok_or_else(|| invalid(format!("{:?} is not a permanent", event.object)))?;
    permanent.copy = Some(CopyEffect::new(
        event.of,
        Arc::clone(&event.definition),
        timestamp,
    ));
    state.next_timestamp += 1;
    note_definition(state, &event.definition);
    Ok(())
}

/// Notes what a definition that enters the game in play can bring that no card of the game had: a triggered, static or
/// replacement ability, a legend, an attachment (`GameState::possible`).
pub(crate) fn note_definition(state: &mut GameState, definition: &CardDefinition) {
    if definition
        .supertypes
        .contains(&mtg_core::card::Supertype::Legendary)
    {
        state.possible.legends = true;
    }
    if crate::attach::definition_attaches(definition) {
        state.possible.attachments = true;
    }
    if crate::ability_reader::definition_triggers(definition) {
        state.triggers_possible = true;
    }
    if crate::ability_reader::definition_has_static(definition) {
        state.statics_possible = true;
    }
    if crate::ability_reader::definition_has_rule(definition) {
        state.rules_possible = true;
    }
    if crate::ability_reader::definition_has_state_trigger(definition) {
        state.possible.states = true;
    }
    if crate::ability_reader::definition_has_replacement(definition) {
        state.possible.replacements = true;
    }
    if crate::ability_reader::definition_has_cost_modifier(definition) {
        state.possible.costs = true;
    }
    if crate::ability_reader::definition_has_protection(definition) {
        state.possible.protection = true;
    }
}

/// Whether the object is a transforming double-faced card with its back face up: copying one waits for the copies of
/// transformed permanents (the roadmap's `n1d4`).
pub(crate) fn is_transformed(object: &GameObject) -> bool {
    object.whole_card().layout == Layout::Transform && object.face().is_some_and(|face| face > 0)
}

/// Whether the object may be copied: an object of the game that is not an ability and not a transformed permanent.
pub(crate) fn may_be_copied(object: &GameObject) -> bool {
    object.kind() != ObjectKind::Ability && !is_transformed(object)
}

/// The copiable values of the object `reference` names (CR 707.2): what its card prints, or the values of the copy
/// effect it is under, as a card of one face. An object that has left since is read from `shot`, its last known
/// information (CR 707.2 and 608.2h); `None` when there is neither, when the object is an ability or when it is a
/// transformed permanent.
pub(crate) fn copiable_values(
    state: &GameState,
    reference: ObjectRef,
    shot: Option<&Snapshot>,
) -> Option<CardDefinition> {
    if let Some(object) = state
        .object(reference.id)
        .filter(|object| object.counter() == reference.counter)
    {
        return may_be_copied(object).then(|| object.card().copiable());
    }
    let shot = shot.filter(|shot| shot.reference() == reference)?;
    if let Some(copy) = &shot.copy {
        return Some(copy.copiable());
    }
    let card = state
        .source_object(reference.id)
        .filter(|card| card.kind() != ObjectKind::Ability)?;
    let whole = card.whole_card();
    let face = shot.face.unwrap_or(0);
    if whole.layout == Layout::Transform && face > 0 {
        return None;
    }
    Some(whole.face(face).copiable())
}
