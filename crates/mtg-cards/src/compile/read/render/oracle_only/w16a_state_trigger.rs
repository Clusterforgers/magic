//! A state trigger: "When you control no enchantments, sacrifice this creature." The condition is written as after
//! "as long as" and the effects as a spell.

use serde_json::Value;

use super::w4d_parts::clauses;
use super::w16a_entry_if::w16a_condition_words;
use crate::ability::Ability;

/// Adds the wordings of a triggered ability whose trigger is a state of the game.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let Ok(value) = serde_json::to_value(ability) else {
        return;
    };
    let Some(inner) = value.get("Triggered").and_then(Value::as_object) else {
        return;
    };
    let plain = inner
        .iter()
        .all(|(key, part)| ["effects", "when"].contains(&key.as_str()) || part.is_null());
    let Some(condition) = inner
        .get("when")
        .and_then(|when| when.get("State"))
        .and_then(|state| state.get("condition"))
    else {
        return;
    };
    let Some(effects) = inner.get("effects").and_then(Value::as_array) else {
        return;
    };
    if !plain {
        return;
    }
    let facts = w16a_condition_words(condition);
    for step in clauses(effects) {
        texts.extend(facts.iter().map(|fact| format!("When {fact}, {step}.")));
    }
}
