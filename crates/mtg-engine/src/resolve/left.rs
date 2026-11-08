//! Fail-soft resolution: a rules error in an ability of a card that runs on a player's read is not the game's to stop on, so
//! the ability is left to the table (`docs/engine.md`, Fail-soft resolution).

use mtg_cards::ability::Ability;
use mtg_cards::read::ability_wording;
use mtg_core::ids::PlayerId;
use serde::{Deserialize, Serialize};

use super::{ResolveError, finish};
use crate::abilities::AbilityObject;
use crate::events::Event;
use crate::object::{GameObject, ObjectRef};
use crate::state::GameState;

/// An ability the game could not run, which its controller and the table resolve by hand: what `AbilityLeftToTable` says.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct LeftToTable {
    /// The object whose spell or ability it is, as it was.
    pub source: ObjectRef,
    /// Who controls it, and so applies it.
    pub controller: PlayerId,
    /// The ability in the words the review sheet showed, empty when nothing words it.
    pub text: String,
}

/// The events that end the resolution of `object`, whose `rules` failed, as one that left the rest to the table: the
/// steps already folded stand, the ability is logged and the spell or ability ends as after any resolution. None while the
/// card does not run on a read and the engine does play what it asks for, so that error stays the game's.
pub(super) fn left_to_table(
    state: &GameState,
    object: &GameObject,
    rules: &Ability,
    error: &ResolveError,
) -> Option<Vec<Event>> {
    let source = object
        .ability()
        .map_or_else(|| object.reference(), AbilityObject::source);
    // What the engine does not play yet is never a reason to stop a game, whoever wrote the card.
    let on_a_read = state
        .source_object(source.id)
        .is_some_and(|holder| holder.card().read);
    if !on_a_read && !matches!(error, ResolveError::Unsupported(_)) {
        return None;
    }
    let mut events = vec![Event::AbilityLeftToTable(Box::new(LeftToTable {
        source,
        controller: object.controller(),
        text: ability_wording(rules).unwrap_or_default(),
    }))];
    events.extend(finish(state, object));
    Some(events)
}
