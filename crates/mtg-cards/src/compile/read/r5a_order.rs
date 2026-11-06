//! The order the writers expect of a selector. A selector is a conjunction, so a rule that lists the same predicates
//! in another order means the same; a rule no writer writes is written again with its selectors in the order the
//! readers give them: adjectives, subtypes, card types, the rest, then the zone and its owner ("basic land card",
//! "creature card from your graveyard"). A list of "or" (`AnyOf`) keeps its order, the order of the words, except a
//! list of card types, which the Oracle text names in alphabetical order.

use serde_json::Value;

use crate::ability::Ability;
use crate::refs::Pred;

/// Where a predicate goes in the order of the readers, by its variant; any other variant is a suffix (rank 4).
const RANKS: &[(&str, u8)] = &[
    ("Spell", 0),
    ("Other", 1),
    ("Supertype", 2),
    ("Color", 2),
    ("Colorless", 2),
    ("Not", 2),
    ("Tapped", 2),
    ("Untapped", 2),
    ("Attacking", 2),
    ("Blocking", 2),
    ("Subtype", 3),
    ("Type", 4),
    ("In", 6),
    ("OwnedBy", 7),
];

/// The variants that make a list of predicates a description of objects, not some other list.
const NOUNS: &[&str] = &[
    "Type",
    "Subtype",
    "Supertype",
    "In",
    "OwnedBy",
    "ControlledBy",
];

/// The keys whose string is the name of a binding: where it is made and where it is read.
const NAMES: &[&str] = &["as", "rest_as", "revealed_as", "Bound", "ChosenAs"];

/// The ability with its selectors in the order of the readers and its numbered bindings ("kept2") under the names
/// the readers give them ("kept"); `None` when nothing changes.
pub fn canonical(ability: &Ability) -> Option<Ability> {
    let mut value = serde_json::to_value(ability).ok()?;
    let moved = reorder(&mut value, false);
    let renamed = rename(&mut value);
    if !moved && !renamed {
        return None;
    }
    serde_json::from_value(value).ok()
}

/// Every string of the value, with whether it sits under a key of [`NAMES`].
fn strings<'a>(value: &'a Value, named: bool, out: &mut Vec<(&'a str, bool)>) {
    match value {
        Value::String(text) => out.push((text, named)),
        Value::Array(list) => list.iter().for_each(|item| strings(item, false, out)),
        Value::Object(map) => {
            for (key, item) in map {
                strings(item, NAMES.contains(&key.as_str()), out);
            }
        }
        _ => {}
    }
}

/// Renames each binding whose name ends in digits to the name without them, when that name is free and the old name
/// appears nowhere but as a binding: a consistent renaming, so the rule means the same. Whether any was renamed.
fn rename(value: &mut Value) -> bool {
    let mut found = Vec::new();
    strings(value, false, &mut found);
    let mut renames: Vec<(String, String)> = Vec::new();
    for &(name, named) in &found {
        let base = name.trim_end_matches(|c: char| c.is_ascii_digit());
        let free = !found.iter().any(|&(other, _)| other == base)
            && !renames.iter().any(|(_, to)| to == base);
        let only_bound = found.iter().all(|&(other, at)| other != name || at);
        if named
            && base.len() < name.len()
            && !base.is_empty()
            && free
            && only_bound
            && !renames.iter().any(|(from, _)| from == name)
        {
            renames.push((name.to_owned(), base.to_owned()));
        }
    }
    for (from, to) in &renames {
        replace(value, from, to, false);
    }
    !renames.is_empty()
}

/// Replaces the binding name `from` by `to` under the keys of [`NAMES`].
fn replace(value: &mut Value, from: &str, to: &str, named: bool) {
    match value {
        Value::String(text) if named && text == from => to.clone_into(text),
        Value::Array(list) => list
            .iter_mut()
            .for_each(|item| replace(item, from, to, false)),
        Value::Object(map) => {
            for (key, item) in map.iter_mut() {
                replace(item, from, to, NAMES.contains(&key.as_str()));
            }
        }
        _ => {}
    }
}

/// The variant name of an externally tagged value.
fn variant(value: &Value) -> Option<&str> {
    match value {
        Value::String(name) => Some(name),
        Value::Object(map) if map.len() == 1 => map.keys().next().map(String::as_str),
        _ => None,
    }
}

/// The rank of a predicate; an `AnyOf` takes the rank of its first member ("artifact or creature card" is a type).
fn rank(pred: &Value) -> u8 {
    let name = variant(pred).unwrap_or_default();
    if name == "AnyOf" {
        return pred
            .get("AnyOf")
            .and_then(Value::as_array)
            .and_then(|members| members.first())
            .map_or(5, rank);
    }
    RANKS
        .iter()
        .find(|(key, _)| *key == name)
        .map_or(5, |(_, rank)| *rank)
}

/// Whether the list is a selector: every member is a predicate and one of them names a kind, a zone or a player.
fn is_selector(list: &[Value]) -> bool {
    list.iter()
        .all(|pred| serde_json::from_value::<Pred>(pred.clone()).is_ok())
        && list
            .iter()
            .any(|pred| variant(pred).is_some_and(|name| NOUNS.contains(&name)))
}

/// Sorts each selector by rank, keeping the order within a rank and leaving the members of an `AnyOf` in place;
/// whether any moved.
fn reorder(value: &mut Value, in_any_of: bool) -> bool {
    let mut moved = false;
    match value {
        Value::Array(list) => {
            if !in_any_of && is_selector(list) && !list.is_sorted_by_key(rank) {
                list.sort_by_key(rank);
                moved = true;
            }
            // The Oracle text names card types in alphabetical order: "artifact or creature card".
            let kind = |pred: &Value| pred.get("Type").and_then(Value::as_str).map(str::to_owned);
            if in_any_of
                && list.iter().all(|pred| kind(pred).is_some())
                && !list.is_sorted_by_key(kind)
            {
                list.sort_by_key(kind);
                moved = true;
            }
            for item in list.iter_mut() {
                moved |= reorder(item, false);
            }
        }
        Value::Object(map) => {
            for (key, item) in map.iter_mut() {
                moved |= reorder(item, key == "AnyOf");
            }
        }
        _ => {}
    }
    moved
}
