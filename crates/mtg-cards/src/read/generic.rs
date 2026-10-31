//! The fallback wording of a node of the rules: its variant and its fields as words, so no node is ever blank.

use serde::Serialize;
use serde_json::{Map, Value};

use super::phrase::{self, Ctx};
use crate::effect::Effect;
use crate::refs::{Ref, Selector};
use crate::value::Amount;

/// A variant name as lowercase words: `DealDamage` is "deal damage", `CantAttack` is "can't attack".
pub(super) fn words(name: &str) -> String {
    let mut out = String::new();
    let mut previous: Option<char> = None;
    for c in name.chars() {
        if c == '_' {
            out.push(' ');
            previous = None;
            continue;
        }
        let after_lower = previous.is_some_and(|p| p.is_ascii_lowercase() || p.is_ascii_digit());
        if c.is_ascii_uppercase() && after_lower {
            out.push(' ');
        }
        out.push(c.to_ascii_lowercase());
        previous = Some(c);
    }
    out.split(' ')
        .map(|word| if word == "cant" { "can't" } else { word })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Whether a string is the name of a variant (`Flying`), as opposed to a name or a number written out.
fn is_variant_name(text: &str) -> bool {
    text.starts_with(|c: char| c.is_ascii_uppercase())
        && text.chars().all(|c| c.is_ascii_alphanumeric())
        && text.chars().any(|c| c.is_ascii_lowercase())
}

/// Any node of the rules as words: a variant by its name and fields, a list by commas.
pub(super) fn node<T: Serialize>(ctx: &Ctx, value: &T) -> String {
    match serde_json::to_value(value) {
        Ok(json) => text(ctx, &json),
        Err(_) => "(unreadable)".to_owned(),
    }
}

/// A JSON value as words.
pub(super) fn text(ctx: &Ctx, value: &Value) -> String {
    match value {
        Value::Null => "nothing".to_owned(),
        Value::Bool(flag) => (if *flag { "yes" } else { "no" }).to_owned(),
        Value::Number(number) => number.to_string(),
        Value::String(name) if is_variant_name(name) => words(name),
        Value::String(other) => other.clone(),
        Value::Array(items) => items
            .iter()
            .map(|item| text(ctx, item))
            .collect::<Vec<_>>()
            .join(", "),
        Value::Object(map) => object(ctx, map),
    }
}

/// An externally tagged variant (`{"Scry": {...}}`) by name and arguments, or a record by its fields.
fn object(ctx: &Ctx, map: &Map<String, Value>) -> String {
    match map.iter().next() {
        Some((name, inner)) if map.len() == 1 && is_variant_name(name) => {
            let head = words(name);
            match inner {
                Value::Object(fields) => {
                    let shown = fields_text(ctx, fields);
                    if shown.is_empty() {
                        head
                    } else {
                        format!("{head} ({shown})")
                    }
                }
                Value::Null => head,
                other => format!("{head} {}", text(ctx, other)),
            }
        }
        _ => fields_text(ctx, map),
    }
}

/// The fields of a record, `who` and `what` first, each by what its name says it holds.
pub(super) fn fields_text(ctx: &Ctx, map: &Map<String, Value>) -> String {
    let first = ["who", "what"]
        .into_iter()
        .filter_map(|key| map.get_key_value(key));
    let rest = map
        .iter()
        .filter(|(key, _)| !matches!(key.as_str(), "who" | "what"));
    first
        .chain(rest)
        .filter_map(|(key, value)| field(ctx, key, value))
        .collect::<Vec<_>>()
        .join(", ")
}

/// One field: a flag by its name, a reference, amount or selector by its phrase, anything else as words.
fn field(ctx: &Ctx, key: &str, value: &Value) -> Option<String> {
    match value {
        Value::Null | Value::Bool(false) => return None,
        Value::Bool(true) => return Some(words(key)),
        Value::Array(items) if items.is_empty() => return None,
        _ => {}
    }
    if matches!(key, "bind" | "revealed_as" | "exiled_as" | "rest_as") {
        return None;
    }
    let typed = typed_phrase(ctx, key, value);
    match (key, typed) {
        ("who" | "what", Some(phrase)) => Some(phrase),
        (_, Some(phrase)) => Some(format!("{} {phrase}", words(key))),
        (_, None) => Some(format!("{} {}", words(key), text(ctx, value))),
    }
}

/// The phrase of a value whose field name says its type.
fn typed_phrase(ctx: &Ctx, key: &str, value: &Value) -> Option<String> {
    match key {
        "who" | "what" | "to" | "from" | "with" | "on" | "of" | "over" | "by" | "at" | "source"
        | "under" | "controller" | "players" | "subject" | "among" => {
            if let Ok(found) = serde_json::from_value::<Ref>(value.clone()) {
                Some(phrase::who(ctx, &found))
            } else {
                serde_json::from_value::<Selector>(value.clone())
                    .ok()
                    .map(|found| phrase::selector(ctx, &found))
            }
        }
        "count" | "amount" | "times" | "per" | "power" | "toughness" | "plus" | "size" => {
            serde_json::from_value::<Amount>(value.clone())
                .ok()
                .map(|found| phrase::amount(ctx, &found))
        }
        "filter" | "selector" => serde_json::from_value::<Selector>(value.clone())
            .ok()
            .map(|found| phrase::selector(ctx, &found)),
        "condition" | "all_if" => serde_json::from_value(value.clone())
            .ok()
            .map(|found| phrase::condition(ctx, &found)),
        "duration" => serde_json::from_value(value.clone())
            .ok()
            .map(|found| phrase::duration(&found)),
        _ => None,
    }
}

/// The keys of an effect's record that hold lists of effects, which become nested steps.
const EFFECT_LISTS: [&str; 10] = [
    "effects",
    "then",
    "otherwise",
    "paid",
    "unpaid",
    "win",
    "lose",
    "first",
    "second",
    "results",
];

/// An effect the typed arms do not word: its variant and plain fields as one line, then each list of effects it holds
/// as nested steps.
pub(super) fn effect(ctx: &Ctx, effect: &Effect, depth: u8, out: &mut Vec<super::Step>) {
    let Ok(json) = serde_json::to_value(effect) else {
        out.push(super::Step::new(depth, "an effect that cannot be shown"));
        return;
    };
    let Value::Object(outer) = &json else {
        out.push(super::Step::new(depth, node(ctx, &json)));
        return;
    };
    let Some((name, Value::Object(body))) = outer.iter().next() else {
        out.push(super::Step::new(depth, node(ctx, &json)));
        return;
    };
    let plain: Map<String, Value> = body
        .iter()
        .filter(|(key, _)| !EFFECT_LISTS.contains(&key.as_str()))
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect();
    let shown = fields_text(ctx, &plain);
    let head = words(name);
    out.push(super::Step::new(
        depth,
        if shown.is_empty() {
            head
        } else {
            format!("{head}: {shown}")
        },
    ));
    for key in EFFECT_LISTS {
        let Some(list) = body.get(key) else { continue };
        let Ok(effects) = serde_json::from_value::<Vec<Effect>>(list.clone()) else {
            if !matches!(list, Value::Array(items) if items.is_empty()) {
                out.push(super::Step::new(
                    depth + 1,
                    format!("{}: {}", words(key), text(ctx, list)),
                ));
            }
            continue;
        };
        if effects.is_empty() {
            continue;
        }
        out.push(super::Step::new(depth + 1, format!("{}:", words(key))));
        for inner in &effects {
            super::effects::push(ctx, inner, depth + 2, out);
        }
    }
}

#[cfg(test)]
mod tests {
    //! Goal: the fallback names a node by its variant and fields in plain words, whatever the node is, and never returns an empty string.
    use rstest::rstest;

    use super::*;

    #[rstest]
    #[case::one_word("Flying", "flying")]
    #[case::two_words("DealDamage", "deal damage")]
    #[case::acronym("ModifyPT", "modify pt")]
    #[case::cant("CantAttack", "can't attack")]
    #[case::digits("W15cClashWon", "w15c clash won")]
    fn variants_become_words(#[case] name: &str, #[case] shown: &str) {
        assert_eq!(words(name), shown);
    }

    #[rstest]
    #[case::unit(r#""Flying""#, "flying")]
    #[case::number("3", "3")]
    #[case::list(r#"["Flying","Reach"]"#, "flying, reach")]
    #[case::tagged_scalar(r#"{"Ward":2}"#, "ward 2")]
    #[case::tagged_record(
        r#"{"Amass":{"subtype":"Zombie","count":2}}"#,
        "amass (count 2, subtype zombie)"
    )]
    #[case::references_by_name(r#"{"Proliferate":{"who":"You"}}"#, "proliferate (you)")]
    #[case::flags(r#"{"Search":{"reveal":true,"reveal_not":false}}"#, "search (reveal)")]
    #[case::null(r"null", "nothing")]
    fn values_become_words(#[case] json: &str, #[case] shown: &str) {
        let value: Value = serde_json::from_str(json).unwrap();
        assert_eq!(text(&Ctx::none(), &value), shown);
    }
}
