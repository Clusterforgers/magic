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

/// [`swapped`] without the wordings of `stand` that call the card "it": the swapped head names another object
/// ("Whenever ~ deals combat damage to a creature"), after which "it" is that object, never the card.
pub(super) fn wf26_swapped_named(stand: &Ability, swaps: &[(&str, &str)]) -> Vec<String> {
    let texts = written_variants(stand);
    let named: Vec<String> = texts
        .iter()
        .filter(|text| !wf26_card_as_it(text, &texts))
        .cloned()
        .collect();
    swapped_in(&named, swaps)
}

/// Whether `text` is another of `texts` with a name of the card ("~", "~'s") said as "it" or "its".
fn wf26_card_as_it(text: &str, texts: &[String]) -> bool {
    let words = [("it", "~"), ("It", "~"), ("its", "~'s"), ("Its", "~'s")];
    words.iter().any(|(pronoun, name)| {
        text.match_indices(pronoun).any(|(at, _)| {
            let end = at + pronoun.len();
            let opens = !text[..at].ends_with(|c: char| c.is_alphanumeric());
            let closes = !text[end..].starts_with(|c: char| c.is_alphanumeric() || c == '\'');
            opens
                && closes
                && texts
                    .iter()
                    .any(|other| *other == format!("{}{name}{}", &text[..at], &text[end..]))
        })
    })
}

/// The `texts` with each `from` replaced once by its `to`; a text without every `from` is left out.
pub(super) fn swapped_in(texts: &[String], swaps: &[(&str, &str)]) -> Vec<String> {
    let mut found: Vec<String> = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for text in texts {
        let mut text = text.clone();
        if swaps.iter().all(|(from, to)| {
            let hit = text.contains(from);
            text = text.replacen(from, to, 1);
            hit
        }) && seen.insert(text.clone())
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
