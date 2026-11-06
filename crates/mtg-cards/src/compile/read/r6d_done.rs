//! "Sacrifice a creature. If you do, ..." (r6d): a step you must take can still take nothing, so an `If` that counts
//! what the step right before it bound at one or more asks "if you do", as after an accepted "may". Only "if you do":
//! the same steps are written alone inside a "may", where a count of none asks about an accepted step that took
//! nothing, not about a refusal.

use serde_json::{Value, json};

/// The step itself when it is one you take without a choice to decline it: no other player and not optional.
pub(super) fn mandatory(effect: &Value) -> Option<Value> {
    let (verb, body) = effect.as_object()?.iter().next()?;
    super::r5b_performed::STEPS
        .contains(&verb.as_str())
        .then_some(())?;
    let yours = body.get("who").is_none_or(|who| who == "You");
    let forced = body
        .get("optional")
        .is_none_or(|optional| optional == false);
    (yours && forced).then(|| effect.clone())
}

/// The condition the writers word, kept after a "may" either way and after a step you must take only as "if you do".
pub(super) fn kept(must: bool, condition: Value) -> Option<Value> {
    (!must || condition == json!("Performed")).then_some(condition)
}
