//! "..., where X is its power": an amount that is a characteristic of an object is written as X in the sentence and named
//! at its end ("Whenever ~ attacks, each other creature you control gets +X/+X until end of turn, where X is ~'s power.").

use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::Value;

use super::amounts::stat_name;
use super::defining;
use super::render;
use super::selectors::stat_word;
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
    /// "the sacrificed creature's power", "the discarded card's mana value": what the cost took.
    Paid,
}

/// What a where clause names.
enum Named {
    /// "its power", "~'s toughness".
    Stat(Whose, StatKind),
    /// "the number of creatures you control", "twice the number of ...", "the total power of ...".
    Counted(Amount),
}

/// ", where X is its power" or ", where X is the number of artifacts you control": what X is, then the end of the
/// sentence.
fn clause(r: &mut Reader<'_>) -> Option<Named> {
    r.punct(',').then_some(())?;
    r.need("where x is")?;
    let found = match r.attempt(characteristic) {
        Some((whose, stat)) => Named::Stat(whose, stat),
        None => Named::Counted(defining::value(r)?),
    };
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
        let stat = paid_stat(r)?;
        return Some((Whose::Paid, stat));
    };
    let stat = stat_word(r)?;
    Some((whose, stat))
}

/// What the cost of an ability or a spell took out of its zone, as a card words it.
const PAID_VERBS: [&str; 3] = ["sacrificed", "discarded", "exiled"];

/// What the cost took, as a card names it.
const PAID_NOUNS: [&str; 7] = [
    "creature",
    "artifact",
    "enchantment",
    "land",
    "permanent",
    "planeswalker",
    "card",
];

/// "the sacrificed creature's power", "the discarded card's mana value": a characteristic of what the cost took, read
/// only where the cost took something (`Reader.paid`).
pub(super) fn paid_stat(r: &mut Reader<'_>) -> Option<StatKind> {
    r.paid.then_some(())?;
    r.attempt(|r| {
        r.need("the")?;
        PAID_VERBS.contains(&r.take_word()?).then_some(())?;
        let noun = r.take_word()?.strip_suffix("'s")?;
        PAID_NOUNS.contains(&noun).then_some(())?;
        stat_word(r)
    })
}

/// Every wording of a characteristic of what the cost took.
fn paid_names(stat: StatKind) -> Vec<String> {
    let Some(name) = stat_name(stat) else {
        return Vec::new();
    };
    PAID_VERBS
        .iter()
        .flat_map(|verb| {
            PAID_NOUNS
                .iter()
                .map(move |noun| format!("the {verb} {noun}'s {name}"))
        })
        .collect()
}

/// "equal to its power", "equal to the number of opponents you have": the characteristic or the count an amount is.
pub(super) fn equal_to(r: &mut Reader<'_>) -> Option<Amount> {
    r.need("equal to")?;
    if let Some((whose, stat)) = r.attempt(characteristic) {
        return Some(Amount::Stat {
            of: object(r, &whose)?,
            stat,
        });
    }
    defining::value(r)
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
        Whose::Paid => Some(Ref::Paid),
        Whose::Its => r.it.clone().or_else(lone_target).or(r.subject.clone()),
        Whose::ThatCreature => r
            .subject
            .clone()
            .filter(|subject| *subject == Ref::Triggering)
            .or_else(|| r.it.clone()),
    }
}

/// Reads a where clause that names a count and gives the count.
pub(super) fn counted(r: &mut Reader<'_>) -> Option<Amount> {
    match clause(r)? {
        Named::Counted(amount) => Some(amount),
        Named::Stat(..) => None,
    }
}

/// Reads the where clause that ends a sentence and puts its amount where the effects have X.
pub(super) fn bind(r: &mut Reader<'_>, effects: &[Effect]) -> Option<Vec<Effect>> {
    let amount = match clause(r)? {
        Named::Stat(whose, stat) => Amount::Stat {
            of: object(r, &whose)?,
            stat,
        },
        Named::Counted(amount) => amount,
    };
    put_in(&effects.to_vec(), &amount)
}

/// The data with the amount in the place of each X; `None` when there is no X.
pub(super) fn put_in<T: Serialize + DeserializeOwned>(data: &T, amount: &Amount) -> Option<T> {
    let amount = serde_json::to_value(amount).ok()?;
    let mut json = serde_json::to_value(data).ok()?;
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

/// The amounts that count, as the data names them.
const COUNTS: [&str; 8] = [
    "Count",
    "Greatest",
    "Total",
    "CardTypes",
    "Sum",
    "Counters",
    "Players",
    "Mutations",
];

/// The counts in the data, each once; what a count holds is not searched.
fn counts(value: &Value, found: &mut Vec<Value>) {
    match value {
        Value::Object(map)
            if map.len() == 1 && map.keys().all(|key| COUNTS.contains(&key.as_str())) =>
        {
            if !found.contains(value) {
                found.push(value.clone());
            }
        }
        Value::Object(map) => map.values().for_each(|child| counts(child, found)),
        Value::Array(items) => items.iter().for_each(|child| counts(child, found)),
        _ => {}
    }
}

/// How a card names a characteristic of an object in a where clause.
fn tails(of: &Ref, stat: StatKind) -> Vec<String> {
    let Some(name) = stat_name(stat) else {
        return Vec::new();
    };
    if *of == Ref::Paid {
        return paid_names(stat);
    }
    let owners: &[&str] = match of {
        Ref::This => &["~'s", "its"],
        Ref::Target(0) => &["its"],
        Ref::Triggering => &["that creature's", "its"],
        _ => &[],
    };
    owners
        .iter()
        .map(|owner| format!("{owner} {name}"))
        .collect()
}

/// How many times a sentence has the amount X (not the `{X}` of a cost).
fn x_count(sentence: &str) -> usize {
    sentence
        .match_indices('X')
        .filter(|(at, _)| {
            let before = sentence[..*at].chars().next_back();
            let after = sentence[at + 1..].chars().next();
            !before.is_some_and(|c| c.is_alphanumeric() || c == '{')
                && !after.is_some_and(|c| c.is_alphanumeric() || c == '}')
        })
        .count()
}

/// The text with each sentence that has X (the text before its period) replaced by the sentences `edit` gives for it,
/// once for each such sentence and each replacement.
fn edit_sentences(text: &str, edit: impl Fn(&str) -> Vec<String>) -> Vec<String> {
    let mut found = Vec::new();
    let mut start = 0;
    for (end, _) in text.match_indices('.') {
        if end + 1 != text.len() && !text[end + 1..].starts_with(' ') {
            continue;
        }
        let sentence = &text[start..end];
        if x_count(sentence) > 0 {
            for edited in edit(sentence) {
                found.push(format!("{}{edited}{}", &text[..start], &text[end..]));
            }
        }
        start = end + 1;
    }
    found
}

/// The text with the tail in front of the period of the sentence that has X, once for each such sentence.
fn insert(text: &str, tail: &str) -> Vec<String> {
    edit_sentences(text, |sentence| vec![format!("{sentence}{tail}")])
}

/// The sentence with `from` replaced by `into`, once.
fn swap(sentence: &str, from: &str, into: &str) -> Option<String> {
    sentence
        .contains(from)
        .then(|| sentence.replacen(from, into, 1))
}

/// The wordings of a sentence with X as the amount of a draw, a mill, a change of life, damage or counters, with the
/// count after "equal to": "draw cards equal to the number of ...", "deals damage equal to ... to any target".
fn equal_to_forms(text: &str, said: &str) -> Vec<String> {
    edit_sentences(text, |sentence| {
        if x_count(sentence) != 1 {
            return Vec::new();
        }
        let mut forms: Vec<String> = Vec::new();
        let verbs: [(&[&str], &str); 2] = [
            (&["draw", "draws", "Draw", "mill", "mills", "Mill"], "cards"),
            (&["lose", "loses", "gain", "gains", "Gain"], "life"),
        ];
        for (verbs, noun) in verbs {
            for verb in verbs {
                forms.extend(swap(
                    sentence,
                    &format!("{verb} X {noun}"),
                    &format!("{verb} {noun} equal to {said}"),
                ));
            }
        }
        if let Some((before, target)) = sentence.split_once(" deals X damage to ") {
            forms.push(format!("{before} deals damage equal to {said} to {target}"));
            forms.push(format!("{before} deals damage to {target} equal to {said}"));
        }
        // The count follows the objects the verb makes: "put a number of +1/+1 counters on ~ equal to ...".
        for verb in ["Put", "put", "Create", "create"] {
            if let Some((before, rest)) = sentence.split_once(&format!("{verb} X ")) {
                forms.push(format!("{before}{verb} a number of {rest} equal to {said}"));
            }
        }
        forms
    })
}

/// The ability with the amount in the data written as X.
fn with_x(mut json: Value, amount: &Value) -> Option<Ability> {
    replace(&mut json, amount, &x()).then_some(())?;
    serde_json::from_value(json).ok()
}

/// The wordings of an ability with one count as its amount: X, with the count named at the end of the sentence that
/// says X ("..., where X is the number of artifacts you control") or after "equal to".
fn counted_wordings(json: &Value) -> Vec<String> {
    let mut found = Vec::new();
    counts(json, &mut found);
    let [count] = found.as_slice() else {
        return Vec::new();
    };
    let Ok(amount) = serde_json::from_value::<Amount>(count.clone()) else {
        return Vec::new();
    };
    let Some(said) = defining::value_text(&amount) else {
        return Vec::new();
    };
    let Some(replaced) = with_x(json.clone(), count) else {
        return Vec::new();
    };
    let tail = format!(", where X is {said}");
    render::variants(&replaced)
        .iter()
        .flat_map(|text| {
            insert(text, &tail)
                .into_iter()
                .chain(equal_to_forms(text, &said))
        })
        .collect()
}

/// The wordings of an ability with one characteristic of an object or one count as its amount, in the sentence that
/// says X and names it at its end. Empty for any other ability.
pub(super) fn wordings(ability: &Ability) -> Vec<String> {
    let Ok(json) = serde_json::to_value(ability) else {
        return Vec::new();
    };
    let mut found = Vec::new();
    stats(&json, &mut found);
    let [stat] = found.as_slice() else {
        return if found.is_empty() {
            counted_wordings(&json)
        } else {
            Vec::new()
        };
    };
    let Ok(Amount::Stat { of, stat: kind }) = serde_json::from_value(stat.clone()) else {
        return Vec::new();
    };
    let Some(replaced) = with_x(json, stat) else {
        return Vec::new();
    };
    let said = tails(&of, kind);
    let texts = render::variants(&replaced);
    let mut found: Vec<String> = texts
        .iter()
        .flat_map(|text| {
            said.iter()
                .flat_map(|said| insert(text, &format!(", where X is {said}")))
        })
        .collect();
    // What a cost took is also worded after "equal to" ("draw cards equal to the sacrificed creature's power").
    if of == Ref::Paid {
        for text in &texts {
            for said in &said {
                found.extend(equal_to_forms(text, said));
            }
        }
    }
    found
}
