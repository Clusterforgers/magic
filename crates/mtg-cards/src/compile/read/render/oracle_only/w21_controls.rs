//! "You control an artifact or enchantment", "you control another artifact creature": at least one permanent a selector
//! names that the player controls.

use super::super::super::selectors::{Form, phrase_text};
use super::super::super::triggers::article;
use crate::refs::{Pred, Ref, Selector};
use serde_json::Value;

/// The words of a comparison that counts at least one permanent the player controls; `None` for any other.
pub(super) fn controls(compare: &Value) -> Option<String> {
    (compare.get("op")? == "Ge" && *compare.get("right")? == 1).then_some(())?;
    let counted: Vec<Pred> =
        serde_json::from_value(compare.pointer("/left/Count")?.clone()).ok()?;
    let mut kinds: Vec<Pred> = counted
        .iter()
        .filter(|pred| **pred != Pred::ControlledBy(Ref::You))
        .cloned()
        .collect();
    (kinds.len() + 1 == counted.len()).then_some(())?;
    let another = kinds.iter().any(|pred| *pred == Pred::Other);
    kinds.retain(|pred| *pred != Pred::Other);
    // "artifact creature": the types may stand in either order.
    let reversed: Vec<Pred> = kinds.iter().rev().cloned().collect();
    let named = phrase_text(&Selector(kinds), Form::Subject)
        .or_else(|| phrase_text(&Selector(reversed), Form::Subject))?;
    let noun = if another {
        format!("another {named}")
    } else {
        format!("{} {named}", article(&named))
    };
    Some(format!("you control {noun}"))
}
