//! Continuous effects that last longer than the turn, which the renderers word only until end of turn: one without
//! end ("That land becomes a 0/0 Elemental creature with haste. It's still a land.") and one until your next turn
//! ("Until your next turn, those creatures can't block."). The ability is written with the effect until end of turn and
//! its one duration is put in the other's place; a group named by the effect before it is "those creatures".

use super::super::written_variants;
use super::w4b_stand::{json, stand};
use crate::ability::Ability;
use serde_json::{Value, json};

const END_OF_TURN: &str = " until end of turn";
const THIS_TURN: &str = " this turn";

/// Adds the wordings of an ability with one effect without end or until your next turn.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let value = json(ability);
    let text = value.to_string();
    if text.contains("\"UntilEndOfTurn\"") {
        return;
    }
    let forever = text.matches("\"duration\":\"Forever\"").count();
    let next = text.matches("\"duration\":\"UntilYourNextTurn\"").count();
    let Some(written) = stand(ability, |value| {
        let from = match (forever, next) {
            (1, 0) => "Forever",
            (0, 1) => "UntilYourNextTurn",
            _ => return None,
        };
        set_duration(value, from)
    }) else {
        return;
    };
    let variants = written_variants(&written);
    if forever == 1 {
        let noun = target_noun(&value);
        for found in variants.iter().filter_map(|text| without_end(text)) {
            if let Some(noun) = &noun {
                texts.push(found.replacen(". It ", &format!(". That {noun} "), 1));
            }
            texts.push(found);
        }
    } else {
        texts.extend(variants.iter().filter_map(|text| until_next_turn(text)));
    }
}

/// Sets the duration `from` of the first effect that has it to until end of turn.
fn set_duration(value: &mut Value, from: &str) -> Option<()> {
    match value {
        Value::Object(map) => {
            if map.get("duration") == Some(&json!(from)) {
                map.insert("duration".to_owned(), json!("UntilEndOfTurn"));
                return Some(());
            }
            map.values_mut().find_map(|inner| set_duration(inner, from))
        }
        Value::Array(items) => items.iter_mut().find_map(|inner| set_duration(inner, from)),
        _ => None,
    }
}

/// The text without its one "until end of turn" at the end of a sentence.
fn without_end(text: &str) -> Option<String> {
    (text.matches(END_OF_TURN).count() == 1 && !text.to_lowercase().contains("until end of turn,"))
        .then_some(())?;
    let at = text.find(END_OF_TURN)?;
    let rest = &text[at + END_OF_TURN.len()..];
    (rest.starts_with('.') || rest.starts_with(',')).then_some(())?;
    Some(format!("{}{rest}", &text[..at]))
}

/// The text with its one sentence "... this turn." opening with "Until your next turn," instead, and "those creatures"
/// for a group the sentence before names ("Goad all creatures your opponents control.").
fn until_next_turn(text: &str) -> Option<String> {
    (text.matches(THIS_TURN).count() == 1 && !text.to_lowercase().contains("until end of turn"))
        .then_some(())?;
    let at = text.find(THIS_TURN)?;
    let rest = &text[at + THIS_TURN.len()..];
    rest.starts_with('.').then_some(())?;
    let start = text[..at].rfind(". ").map_or(0, |found| found + 2);
    let (head, sentence) = (&text[..start], &text[start..at]);
    let mut chars = sentence.chars();
    let first = chars.next()?;
    let sentence = format!("{}{}", first.to_lowercase(), chars.as_str());
    let sentence = those(head, &sentence).unwrap_or(sentence);
    Some(format!("{head}Until your next turn, {sentence}{rest}"))
}

/// The sentence with "those creatures" for its subject when the sentence before names all of them.
fn those(head: &str, sentence: &str) -> Option<String> {
    let verb = sentence
        .find(" can't ")
        .or_else(|| sentence.find(" don't "))?;
    let subject = &sentence[..verb];
    head.contains(&format!("all {subject}")).then_some(())?;
    let noun = subject.split(' ').next()?;
    Some(format!("those {noun}{}", &sentence[verb..]))
}

/// The card type, in lower case, of the first target of the ability ("land" for "target land you control").
fn target_noun(value: &Value) -> Option<String> {
    let body = value.as_object()?.values().next()?;
    let types: Vec<&str> = body
        .pointer("/targets/0/what/Object")?
        .as_array()?
        .iter()
        .filter_map(|pred| pred.get("Type").and_then(Value::as_str))
        .collect();
    match types.as_slice() {
        [only] => Some(only.to_lowercase()),
        _ => None,
    }
}
