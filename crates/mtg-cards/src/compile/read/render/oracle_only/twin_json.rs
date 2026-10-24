//! The twin of an ability with one part changed, made on its JSON: the writers that derive an oracle wording from the
//! plain wording of a simpler ability use it.

use crate::ability::Ability;
use serde_json::Value;

/// The ability with `change` applied to its JSON; `None` when `change` finds nothing to change or the result is no ability.
pub(super) fn twin(
    ability: &Ability,
    change: impl FnOnce(&mut Value) -> Option<()>,
) -> Option<Ability> {
    let mut value = serde_json::to_value(ability).ok()?;
    change(&mut value)?;
    serde_json::from_value(value).ok()
}

/// The body of a spell, a triggered ability or an activated ability.
pub(super) fn inner_mut(value: &mut Value) -> Option<&mut Value> {
    let key = ["Spell", "Triggered", "Activated"]
        .into_iter()
        .find(|key| value.get(key).is_some())?;
    value.get_mut(key)
}

/// The text with its first letter in lower case: a sentence that follows an introduction.
pub(super) fn lowered(text: &str) -> String {
    let mut chars = text.chars();
    chars
        .next()
        .map(|first| first.to_lowercase().chain(chars).collect())
        .unwrap_or_default()
}
