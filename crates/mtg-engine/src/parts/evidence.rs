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

/// The total a pick of `R6703TotalValue` reaches: the mana value of the objects together, or the mana symbols of the
/// color among their mana costs (a hybrid symbol counts once for each of its colors).
pub(super) fn p160_total(
    state: &GameState,
    objects: &[ObjectId],
    symbols: Option<mtg_core::color::Color>,
) -> u32 {
    let Some(color) = symbols else {
        return total_value(state, objects);
    };
    let counted = objects
        .iter()
        .filter_map(|object| state.object(*object)?.mana_cost())
        .flat_map(|cost| cost.symbols().iter())
        .filter(|symbol| symbol.colors().contains(color))
        .count();
    u32::try_from(counted).unwrap_or(u32::MAX)
}

/// "Collect evidence 4: exile cards from your graveyard with total mana value 4 or more."
pub(super) fn text(value: usize) -> String {
    format!(
        "Collect evidence {value}: exile cards from your graveyard with total mana value {value} or more."
    )
}

/// "Exile any number of historic cards from your graveyard with total mana value 30 or more."
pub(super) fn r6703_text(
    filter: &mtg_cards::refs::Selector,
    value: usize,
    symbols: Option<mtg_core::color::Color>,
) -> String {
    let (head, _) = crate::wording::phrase(filter);
    let kind = if head == "permanent" {
        "card".to_owned()
    } else {
        format!("{head} card")
    };
    match symbols {
        None => format!(
            "Exile any number of {kind}s from your graveyard with total mana value {value} or more."
        ),
        Some(color) => format!(
            "Exile any number of {kind}s from your graveyard with {value} or more {} mana symbols among their mana costs.",
            format!("{color:?}").to_lowercase()
        ),
    }
}
