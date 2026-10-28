//! A permanent that becomes a creature, its changes in any order ("This land becomes a 3/3 Citizen creature with
//! vigilance until end of turn. It's still a land."): the changes are written in the order the renderers read them,
//! and a land that keeps its types says "It's still a land." for "in addition to its other types" (CR 205.1b).

use super::super::written_variants;
use super::w4b_stand::{json, stand};
use crate::ability::Ability;
use serde_json::{Value, json};

const IN_ADDITION: &str = " in addition to its other types";
const STILL_A_LAND: &str = "It's still a land.";

/// Adds the wordings of an ability that makes a permanent a creature.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let value = json(ability);
    if !value.to_string().contains(r#"{"AddType":"Creature"}"#) {
        return;
    }
    let mut sorted = value.clone();
    let base = if sort_mods(&mut sorted) {
        let Some(written) = stand(ability, |value| {
            *value = sorted;
            Some(())
        }) else {
            return;
        };
        let base = written_variants(&written);
        texts.extend(base.iter().cloned());
        base
    } else {
        texts.clone()
    };
    if !targets_land(&value) && !base.iter().any(|text| text.contains(STILL_A_LAND)) {
        return;
    }
    let lands: Vec<String> = base.iter().filter_map(|text| still_a_land(text)).collect();
    let joined: Vec<String> = lands.iter().filter_map(|text| each_and(text)).collect();
    texts.extend(lands);
    texts.extend(joined);
}

/// The text with "for each legendary creature and planeswalker you control" for the "or" of a count of either kind.
fn each_and(text: &str) -> Option<String> {
    let start = text.find(" for each ")?;
    let end = start + text[start..].find(" you control")?;
    let phrase = &text[start..end];
    (phrase.matches(" or ").count() == 1).then_some(())?;
    Some(format!(
        "{}{}{}",
        &text[..start],
        phrase.replacen(" or ", " and ", 1),
        &text[end..]
    ))
}

/// Sorts each list of changes that makes a permanent a creature into the order the renderers read; true when one
/// changed.
fn sort_mods(value: &mut Value) -> bool {
    match value {
        Value::Object(map) => {
            let mut changed = false;
            for (key, inner) in map.iter_mut() {
                if key == "mods" {
                    if let Some(mods) = inner.as_array_mut() {
                        let before = mods.clone();
                        if mods.contains(&json!({"AddType": "Creature"})) {
                            mods.sort_by_key(rank);
                        }
                        changed |= *mods != before;
                    }
                } else {
                    changed |= sort_mods(inner);
                }
            }
            changed
        }
        Value::Array(items) => items
            .iter_mut()
            .fold(false, |changed, item| sort_mods(item) | changed),
        _ => false,
    }
}

/// The place of a change in "becomes a 3/3 red Elemental artifact creature with haste".
fn rank(modification: &Value) -> u8 {
    let name = modification
        .as_object()
        .and_then(|map| map.keys().next().cloned())
        .unwrap_or_default();
    match (
        name.as_str(),
        modification.get("AddType").and_then(Value::as_str),
    ) {
        ("SetPT", _) => 0,
        ("SetColors", _) => 1,
        ("SetCreatureTypes", _) => 2,
        ("AddSubtype", _) => 3,
        (_, Some("Artifact")) => 4,
        (_, Some("Creature")) => 5,
        _ => 6,
    }
}

/// Whether the first target of the ability is a land.
fn targets_land(value: &Value) -> bool {
    value
        .as_object()
        .and_then(|map| map.values().next())
        .and_then(|body| body.pointer("/targets/0/what/Object"))
        .and_then(Value::as_array)
        .is_some_and(|preds| preds.contains(&json!({"Type": "Land"})))
}

/// The text with "in addition to its other types" in a sentence that makes a land a creature replaced by "It's still a
/// land." after it.
fn still_a_land(text: &str) -> Option<String> {
    let at = text.find(IN_ADDITION)?;
    let (head, tail) = (&text[..at], &text[at + IN_ADDITION.len()..]);
    if !head.ends_with(" creature") && !head.contains(" creature with ") {
        return None;
    }
    if tail.contains(STILL_A_LAND) {
        return Some(format!("{head}{tail}"));
    }
    let end = tail.find('.')?;
    Some(format!(
        "{head}{}. {STILL_A_LAND}{}",
        &tail[..end],
        &tail[end + 1..]
    ))
}
