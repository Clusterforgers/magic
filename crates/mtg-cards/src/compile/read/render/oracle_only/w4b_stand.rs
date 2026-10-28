//! The stand-in the continuous-effect writers share: the ability is written with a known-good part in place of the one
//! a writer words ("it's your turn" for another condition, flying for another keyword), and the words of that part are
//! swapped for the writer's in each wording.

use super::super::written_variants;
use crate::ability::Ability;
use serde_json::Value;

/// The ability as JSON, which the writers match and edit.
pub(super) fn json<T: serde::Serialize + ?Sized>(value: &T) -> Value {
    serde_json::to_value(value).unwrap_or(Value::Null)
}

/// The ability with `edit` applied to its JSON, when the edit applies and the result is an ability.
pub(super) fn stand(
    ability: &Ability,
    edit: impl FnOnce(&mut Value) -> Option<()>,
) -> Option<Ability> {
    let mut value = json(ability);
    edit(&mut value)?;
    serde_json::from_value(value).ok()
}

/// The wordings of `stand` with each `from` replaced once by its `to`; a wording without every `from` is left out.
pub(super) fn swapped(stand: &Ability, swaps: &[(&str, &str)]) -> Vec<String> {
    swapped_in(&written_variants(stand), swaps)
}

/// The `texts` with each `from` replaced once by its `to`; a text without every `from` is left out.
pub(super) fn swapped_in(texts: &[String], swaps: &[(&str, &str)]) -> Vec<String> {
    let mut found: Vec<String> = Vec::new();
    for text in texts {
        let mut text = text.clone();
        if swaps.iter().all(|(from, to)| {
            let hit = text.contains(from);
            text = text.replacen(from, to, 1);
            hit
        }) && !found.contains(&text)
        {
            found.push(text);
        }
    }
    found
}

/// The count word of a JSON number ("three"), for a number from one to twenty.
pub(super) fn word(value: &Value) -> Option<&'static str> {
    let number = u32::try_from(value.as_u64()?).ok()?;
    super::super::super::amounts::count_word(number)
}
