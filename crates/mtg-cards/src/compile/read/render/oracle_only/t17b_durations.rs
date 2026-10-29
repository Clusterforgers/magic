//! Continuous effects with a duration the pump writers do not word: until end of combat, until the end of your next
//! turn, for as long as the card remains on the battlefield or you control it. The ability is written until end of turn
//! and the words of the duration are put in their place.

use super::super::written_variants;
use super::w4b_stand::{json, stand};
use crate::ability::Ability;
use serde_json::{Value, json};

const END_OF_TURN: &str = "until end of turn";

/// A duration, its words, and whether the words may also open the sentence.
const DURATIONS: [(&str, &str, bool); 4] = [
    ("UntilEndOfCombat", "until end of combat", true),
    (
        "UntilEndOfYourNextTurn",
        "until the end of your next turn",
        true,
    ),
    (
        "WhileSourceOnBattlefield",
        "for as long as ~ remains on the battlefield",
        false,
    ),
    (
        "WhileYouControlSource",
        "for as long as you control ~",
        false,
    ),
];

/// Adds the wordings of an ability whose one continuous effect has one of the durations.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let value = json(ability);
    if value.to_string().contains("\"UntilEndOfTurn\"") {
        return;
    }
    for (name, words, leading) in DURATIONS {
        if continuous_with(&value, name) != 1 {
            continue;
        }
        let Some(written) = stand(ability, |value| set_duration(value, name)) else {
            continue;
        };
        texts.extend(
            written_variants(&written)
                .iter()
                .filter_map(|text| swap(text, words, leading)),
        );
    }
}

/// How many continuous effects in the JSON last for the duration.
fn continuous_with(value: &Value, name: &str) -> usize {
    match value {
        Value::Object(map) => map
            .iter()
            .map(|(key, inner)| {
                let here = key == "Continuous" && inner.get("duration") == Some(&json!(name));
                usize::from(here) + continuous_with(inner, name)
            })
            .sum(),
        Value::Array(items) => items.iter().map(|inner| continuous_with(inner, name)).sum(),
        _ => 0,
    }
}

/// Sets the duration of the continuous effect that has it to until end of turn.
fn set_duration(value: &mut Value, name: &str) -> Option<()> {
    match value {
        Value::Object(map) => {
            if let Some(Value::Object(inner)) = map.get_mut("Continuous")
                && inner.get("duration") == Some(&json!(name))
            {
                inner.insert("duration".to_owned(), json!("UntilEndOfTurn"));
                return Some(());
            }
            map.values_mut().find_map(|inner| set_duration(inner, name))
        }
        Value::Array(items) => items.iter_mut().find_map(|inner| set_duration(inner, name)),
        _ => None,
    }
}

/// The text with its one "until end of turn" worded as `words`: at the end of the sentence, or opening it when
/// `leading`. A text with another "this turn" keeps the turn in another part and is left out.
fn swap(text: &str, words: &str, leading: bool) -> Option<String> {
    let lowered = text.to_ascii_lowercase();
    (lowered.matches(END_OF_TURN).count() == 1 && !lowered.contains(" this turn")).then_some(())?;
    let at = lowered.find(END_OF_TURN)?;
    let after = &text[at + END_OF_TURN.len()..];
    let before = &text[..at];
    if after.starts_with(',') {
        return leading.then(|| {
            let opening = before.is_empty() || before.ends_with(": ") || before.ends_with(". ");
            let words = if opening {
                super::super::capitalize(words)
            } else {
                words.to_owned()
            };
            format!("{before}{words}{after}")
        });
    }
    let trailing = before.ends_with(' ') && (after.is_empty() || after.starts_with('.'));
    trailing.then(|| format!("{before}{words}{after}"))
}
