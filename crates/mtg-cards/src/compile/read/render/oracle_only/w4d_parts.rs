//! Shared parts of the writers of delayed triggers and of steps repeated for each player: the wordings of an ability with
//! other effects, its head before the effects, a reference swapped for `~`, and a player's action said in the third
//! person.

use serde_json::{Value, json};

use super::super::written_variants;
use super::twin_json::inner_mut;
use crate::ability::Ability;

/// The JSON of the ability and a copy of its effects; `None` for an ability without effects.
pub(super) fn parts(ability: &Ability) -> Option<(Value, Vec<Value>)> {
    let mut value = serde_json::to_value(ability).ok()?;
    let effects = inner_mut(&mut value)?.get("effects")?.as_array()?.clone();
    Some((value, effects))
}

/// The wordings of the ability with `effects` in place of its own.
pub(super) fn with_effects(value: &Value, effects: &[Value]) -> Vec<String> {
    let mut value = value.clone();
    let Some(inner) = inner_mut(&mut value).and_then(Value::as_object_mut) else {
        return Vec::new();
    };
    inner.insert("effects".to_owned(), Value::Array(effects.to_vec()));
    wordings(value)
}

/// The wordings the checker accepts for an ability given as JSON; empty when the JSON is no ability.
pub(super) fn wordings(value: Value) -> Vec<String> {
    serde_json::from_value::<Ability>(value)
        .map(|ability| written_variants(&ability))
        .unwrap_or_default()
}

/// The wordings of a spell with these effects, without the closing period and with a lower-case first letter.
pub(super) fn clauses(effects: &[Value]) -> Vec<String> {
    wordings(json!({"Spell": {"effects": effects}}))
        .iter()
        .filter_map(|text| text.strip_suffix('.'))
        .map(super::twin_json::lowered)
        .collect()
}

/// The heads of the ability before its effects: "" for a spell, its cost ("{T}: ") or its trigger ("When ~ enters, ").
pub(super) fn heads(value: &Value) -> Vec<String> {
    let mut value = value.clone();
    let Some(inner) = inner_mut(&mut value).and_then(Value::as_object_mut) else {
        return Vec::new();
    };
    inner.remove("targets");
    inner.insert("effects".to_owned(), json!([{"Draw": {}}]));
    let mut heads: Vec<String> = wordings(value)
        .iter()
        .filter_map(|text| {
            text.strip_suffix("Draw a card.")
                .or_else(|| text.strip_suffix("draw a card."))
        })
        .map(str::to_owned)
        .collect();
    heads.dedup();
    heads
}

/// The head and a sentence joined: a sentence after a cost or alone opens with a capital, after a trigger in lower case.
pub(super) fn joined(head: &str, sentence: &str) -> String {
    if head.ends_with(", ") {
        format!("{head}{}", super::twin_json::lowered(sentence))
    } else {
        format!("{head}{}", capitalized(sentence))
    }
}

/// The text with its first letter in upper case.
pub(super) fn capitalized(text: &str) -> String {
    let mut chars = text.chars();
    chars
        .next()
        .map(|first| first.to_uppercase().chain(chars).collect())
        .unwrap_or_default()
}

/// The JSON with every `from` (and every `{"Is": from}`) replaced by `to`; whether anything was replaced.
pub(super) fn swapped(value: &mut Value, from: &Value, to: &Value) -> bool {
    if *value == *from || value.get("Is") == Some(from) {
        *value = to.clone();
        return true;
    }
    match value {
        Value::Array(items) => items
            .iter_mut()
            .fold(false, |any, item| swapped(item, from, to) | any),
        Value::Object(map) => map
            .values_mut()
            .fold(false, |any, item| swapped(item, from, to) | any),
        _ => false,
    }
}

/// The verbs a player's action opens with, in the second person.
const VERBS: [&str; 15] = [
    "search",
    "put",
    "shuffle",
    "lose",
    "gain",
    "draw",
    "discard",
    "sacrifice",
    "create",
    "return",
    "mill",
    "exile",
    "reveal",
    "get",
    "may",
];

/// A player's action written in the second person ("You lose 2 life", "search your library ..., then shuffle") said of
/// `subject` in the third person ("each player loses 2 life"); `None` when it opens with no known verb.
pub(super) fn third_person(text: &str, subject: &str) -> Option<String> {
    let text = text.strip_suffix('.').unwrap_or(text);
    let text = super::twin_json::lowered(text);
    let text = text.strip_prefix("you ").unwrap_or(&text);
    let mut words: Vec<String> = Vec::new();
    for word in text.split(' ') {
        let opens = words
            .last()
            .is_none_or(|last| last.ends_with(',') || last == "then" || last == "and");
        let said = match word {
            "you" => "they".to_owned(),
            "your" => "their".to_owned(),
            _ if opens && VERBS.contains(&word) => conjugated(word),
            _ if words.is_empty() => return None,
            _ => word.to_owned(),
        };
        words.push(said);
    }
    Some(format!("{subject} {}", words.join(" ")))
}

/// The verb in the third person singular.
fn conjugated(verb: &str) -> String {
    match verb {
        "may" => verb.to_owned(),
        _ if verb.ends_with("ch") || verb.ends_with("sh") => format!("{verb}es"),
        _ => format!("{verb}s"),
    }
}
