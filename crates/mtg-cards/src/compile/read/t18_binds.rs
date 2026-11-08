//! Bookkeeping names of the translator: a second use of a name is numbered ("kept2", "rest3"), which no writer expects.

use std::collections::BTreeMap;

use serde_json::Value;

use crate::ability::Ability;

/// The name without its trailing digits when it is a numbered bookkeeping name ("kept2" is "kept").
fn plain(name: &str) -> Option<&str> {
    let stem = name.trim_end_matches(|c: char| c.is_ascii_digit());
    (stem.len() < name.len()
        && !stem.is_empty()
        && stem.chars().all(|c| c.is_ascii_lowercase() || c == '_'))
    .then_some(stem)
}

/// Every string of the value, and those that are numbered names.
fn collect(value: &Value, names: &mut Vec<String>, all: &mut Vec<String>) {
    match value {
        Value::String(text) => {
            all.push(text.clone());
            if plain(text).is_some() {
                names.push(text.clone());
            }
        }
        Value::Array(items) => items.iter().for_each(|item| collect(item, names, all)),
        Value::Object(map) => map.values().for_each(|item| collect(item, names, all)),
        _ => {}
    }
}

fn rename(value: &mut Value, to: &BTreeMap<String, String>) {
    match value {
        Value::String(text) => {
            if let Some(new) = to.get(text.as_str()) {
                text.clone_from(new);
            }
        }
        Value::Array(items) => items.iter_mut().for_each(|item| rename(item, to)),
        Value::Object(map) => map.values_mut().for_each(|item| rename(item, to)),
        _ => {}
    }
}

/// The ability with its numbered names unnumbered, when that keeps the names apart: two names that become one, or a
/// name that becomes one already in use, leave the ability as it is (`None`).
pub(super) fn t18_unnumbered(ability: &Ability) -> Option<Ability> {
    let mut value = serde_json::to_value(ability).ok()?;
    let (mut numbered, mut all) = (Vec::new(), Vec::new());
    collect(&value, &mut numbered, &mut all);
    numbered.sort();
    numbered.dedup();
    if numbered.is_empty() {
        return None;
    }
    let mut to = BTreeMap::new();
    for name in &numbered {
        let stem = plain(name)?.to_owned();
        let taken = all.contains(&stem) || to.values().any(|seen| *seen == stem);
        if taken {
            return None;
        }
        to.insert(name.clone(), stem);
    }
    rename(&mut value, &to);
    serde_json::from_value(value).ok()
}
