//! Collecting evidence as a cost (CR 701.59): cards from the payer's graveyard with total mana value at least `N` are exiled.

use mtg_core::ids::ObjectId;

use crate::state::GameState;

/// The mana value of a card, 0 for an object that is gone.
pub(super) fn value_of(state: &GameState, object: ObjectId) -> u32 {
    state
        .object(object)
        .map_or(0, crate::object::GameObject::mana_value)
}

/// The mana value of all of `objects` together.
pub(super) fn total_value(state: &GameState, objects: &[ObjectId]) -> u32 {
    objects
        .iter()
        .map(|object| value_of(state, *object))
        .fold(0, u32::saturating_add)
}

/// "Collect evidence 4: exile cards from your graveyard with total mana value 4 or more."
pub(super) fn text(value: usize) -> String {
    format!(
        "Collect evidence {value}: exile cards from your graveyard with total mana value {value} or more."
    )
}
