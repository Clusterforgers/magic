//! "..., where X is its power": an amount that is a characteristic of an object is written as X in the sentence and named
//! at its end ("Whenever ~ attacks, each other creature you control gets +X/+X until end of turn, where X is ~'s power.").

use serde_json::Value;

use super::amounts::stat_name;
use super::render;
use super::tokens::Reader;
use crate::ability::Ability;
use crate::effect::Effect;
use crate::refs::{Ref, Targetable};
use crate::value::{Amount, StatKind};

/// Whose characteristic a where clause names, as it words it.
enum Whose {
    /// "its power".
    Its,
    /// "this creature's power", "~'s power".
    Card,
    /// "that creature's power".
    ThatCreature,
}

/// ", where X is its power": the object and the characteristic, then the end of the sentence.
fn clause(r: &mut Reader<'_>) -> Option<(Whose, StatKind)> {
    r.punct(',').then_some(())?;
    r.need("where x is")?;
    let found = characteristic(r)?;
    (r.at_punct('.') || r.end()).then_some(found)
}

/// "its power", "this creature's toughness", "that creature's mana value": whose and which characteristic.
fn characteristic(r: &mut Reader<'_>) -> Option<(Whose, StatKind)> {
    let whose = if r.word("its") {
        Whose::Its
    } else if r.source() {
        (r.punct('\'') && r.word("s")).then_some(())?;
        Whose::Card
    } else if r.words("that creature's") {
        Whose::ThatCreature
    } else {
        return None;
    };
    let stat = if r.word("power") {
        StatKind::Power
    } else if r.word("toughness") {
        StatKind::Toughness
    } else if r.words("mana value") {
        StatKind::ManaValue
    } else {
        return None;
    };
    Some((whose, stat))
}

/// "equal to its power": the characteristic a count of tokens is.
pub(super) fn equal_to(r: &mut Reader<'_>) -> Option<Amount> {
    r.need("equal to")?;
    let (whose, stat) = characteristic(r)?;
    Some(Amount::Stat {
        of: object(r, &whose)?,
        stat,
    })
}

/// Whether the rest of the sentence is a where clause that names a characteristic, read ahead without moving.
pub(super) fn ahead(r: &mut Reader<'_>) -> bool {
    r.lookahead(|r| {
        while !r.end() && !r.at_punct('.') {
            if r.attempt(clause).is_some() {
                return Some(());
            }
            r.skip();
        }
        None
    })
    .is_some()
}

/// The object a where clause names: the sentence's own "it", the one target, or the object of the ability's event.
fn object(r: &Reader<'_>, whose: &Whose) -> Option<Ref> {
    let lone_target = || {
        let [spec] = r.targets.as_slice() else {
            return None;
        };
        matches!(spec.what, Targetable::Object(_)).then_some(Ref::Target(0))
    };
    match whose {
        Whose::Card => Some(Ref::This),
        Whose::Its => r.it.clone().or_else(lone_target).or(r.subject.clone()),
        Whose::ThatCreature => r
            .subject
            .clone()
            .filter(|subject| *subject == Ref::Triggering)
            .or_else(|| r.it.clone()),
    }
}

/// Reads the where clause that ends a sentence and puts its amount where the effects have X.
pub(super) fn bind(r: &mut Reader<'_>, effects: &[Effect]) -> Option<Vec<Effect>> {
    let (whose, stat) = clause(r)?;
    let of = object(r, &whose)?;
    let amount = serde_json::to_value(Amount::Stat { of, stat }).ok()?;
    let mut json = serde_json::to_value(effects).ok()?;
    replace(&mut json, &x(), &amount).then_some(())?;
    serde_json::from_value(json).ok()
}

/// The announced X in the data.
fn x() -> Value {
    serde_json::to_value(Amount::X).unwrap_or(Value::Null)
}

/// Puts `into` in the place of every `from` in the data; whether there was one.
fn replace(value: &mut Value, from: &Value, into: &Value) -> bool {
    if value == from {
        value.clone_from(into);
        return true;
    }
    let children: Box<dyn Iterator<Item = &mut Value>> = match value {
        Value::Array(items) => Box::new(items.iter_mut()),
        Value::Object(map) => Box::new(map.values_mut()),
        _ => return false,
    };
    children.fold(false, |found, child| replace(child, from, into) | found)
}

/// The characteristics of objects in the data, each once.
fn stats(value: &Value, found: &mut Vec<Value>) {
    match value {
        Value::Object(map) if map.len() == 1 && map.contains_key("Stat") => {
            if !found.contains(value) {
                found.push(value.clone());
            }
        }
        Value::Object(map) => map.values().for_each(|child| stats(child, found)),
        Value::Array(items) => items.iter().for_each(|child| stats(child, found)),
        _ => {}
    }
}

/// The wordings of a where clause for an object: how a card names it.
fn tails(of: &Ref, stat: StatKind) -> Vec<String> {
    let Some(name) = stat_name(stat) else {
        return Vec::new();
    };
    let owners: &[&str] = match of {
        Ref::This => &["~'s", "its"],
        Ref::Target(0) => &["its"],
        Ref::Triggering => &["that creature's", "its"],
        _ => &[],
    };
    owners
        .iter()
        .map(|owner| format!(", where X is {owner} {name}"))
        .collect()
}

/// Whether a sentence has the amount X (not the `{X}` of a cost).
fn has_x(sentence: &str) -> bool {
    sentence.match_indices('X').any(|(at, _)| {
        let before = sentence[..at].chars().next_back();
        let after = sentence[at + 1..].chars().next();
        !before.is_some_and(|c| c.is_alphanumeric() || c == '{')
            && !after.is_some_and(|c| c.is_alphanumeric() || c == '}')
    })
}

/// The text with the tail in front of the period of the sentence that has X, once for each such sentence.
fn insert(text: &str, tail: &str) -> Vec<String> {
    let mut found = Vec::new();
    let mut start = 0;
    for (end, _) in text.match_indices('.') {
        if end + 1 != text.len() && !text[end + 1..].starts_with(' ') {
            continue;
        }
        if has_x(&text[start..end]) {
            found.push(format!("{}{tail}{}", &text[..end], &text[end..]));
        }
        start = end + 1;
    }
    found
}

/// The wordings of an ability with one characteristic of an object as its amount, in the sentence that says X and
/// names the characteristic at its end. Empty for any other ability.
pub(super) fn wordings(ability: &Ability) -> Vec<String> {
    let Ok(mut json) = serde_json::to_value(ability) else {
        return Vec::new();
    };
    let mut found = Vec::new();
    stats(&json, &mut found);
    let [stat] = found.as_slice() else {
        return Vec::new();
    };
    let Ok(Amount::Stat { of, stat: kind }) = serde_json::from_value(stat.clone()) else {
        return Vec::new();
    };
    replace(&mut json, stat, &x());
    let Ok(replaced) = serde_json::from_value::<Ability>(json) else {
        return Vec::new();
    };
    let tails = tails(&of, kind);
    render::variants(&replaced)
        .iter()
        .flat_map(|text| tails.iter().flat_map(|tail| insert(text, tail)))
        .collect()
}
